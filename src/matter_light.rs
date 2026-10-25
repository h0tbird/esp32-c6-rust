use core::cell::Cell;

use defmt::{error, info, warn};
use embassy_futures::select::select;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, signal::Signal};
use embassy_time::Timer;
use rs_matter_embassy::matter::{
    dm::{
        Cluster,
        clusters::app::on_off::{self, EffectVariantEnum, OnOffHooks, StartUpOnOffEnum},
    },
    error::{Error, ErrorCode},
    persist::{KvBlobStoreAccess, VENDOR_KEYS_START},
    tlv::Nullable,
    with,
};
use smart_leds::{RGB8, SmartLedsWrite};

use crate::light::{BlinkSequence, HALF_PERIOD_MS, LightSettings};

// The pinned Thread transport owns VENDOR_KEYS_START for its SRP ECDSA key.
const THREAD_SRP_SETTINGS_KEY: u16 = VENDOR_KEYS_START;
const LIGHT_SETTINGS_KEY: u16 = VENDOR_KEYS_START + 1;

pub const LIGHT_CLUSTER: Cluster<'static> = on_off::FULL_CLUSTER
    .with_revision(6)
    .with_features(on_off::Feature::LIGHTING.bits())
    .with_attrs(with!(
        required;
        on_off::AttributeId::OnOff
            | on_off::AttributeId::GlobalSceneControl
            | on_off::AttributeId::OnTime
            | on_off::AttributeId::OffWaitTime
            | on_off::AttributeId::StartUpOnOff
    ))
    .with_cmds(with!(
        on_off::CommandId::Off
            | on_off::CommandId::On
            | on_off::CommandId::Toggle
            | on_off::CommandId::OffWithEffect
            | on_off::CommandId::OnWithRecallGlobalScene
            | on_off::CommandId::OnWithTimedOff
    ));

pub struct BoardLight<'a, K> {
    kv: &'a K,
    settings: Cell<LightSettings>,
    changed: Signal<NoopRawMutex, ()>,
}

impl<'a, K: KvBlobStoreAccess> BoardLight<'a, K> {
    pub fn load(kv: &'a K) -> Result<Self, Error> {
        let settings = kv
            .access(|store, scratch| {
                let current = match store.load(LIGHT_SETTINGS_KEY, scratch)? {
                    Some(bytes) => {
                        let settings = LightSettings::decode(bytes).ok_or_else(|| {
                            error!("Invalid LED settings at key 0x1001 ({} bytes)", bytes.len());
                            Error::from(ErrorCode::Invalid)
                        })?;
                        info!("Loaded persistent LED settings from key 0x1001");
                        Some(settings)
                    }
                    None => None,
                };
                let (legacy, thread_record_present) =
                    match store.load(THREAD_SRP_SETTINGS_KEY, scratch)? {
                        Some(bytes) => (LightSettings::decode(bytes), true),
                        None => (None, false),
                    };
                let settings = current.or(legacy).unwrap_or_default();
                if current.is_none() {
                    if legacy.is_some() {
                        warn!("Migrating legacy LED settings from key 0x1000 to 0x1001");
                    } else if thread_record_present {
                        warn!("No separate LED settings; initializing off, preserving Thread data");
                    } else {
                        info!("Initializing persistent LED settings: off");
                    }
                    store.store(LIGHT_SETTINGS_KEY, &settings.encode(), scratch)?;
                }
                if legacy.is_some() {
                    // Save first so an interrupted migration can be retried without losing state.
                    warn!(
                        "Removing legacy LED record from Thread SRP key; SRP key must regenerate"
                    );
                    store.remove(THREAD_SRP_SETTINGS_KEY, scratch)?;
                }
                Ok(settings)
            })
            .inspect_err(|err: &Error| error!("LED settings storage failed: {:?}", err))?;
        Ok(Self {
            kv,
            settings: Cell::new(settings),
            changed: Signal::new(),
        })
    }

    fn save(&self, settings: LightSettings) -> Result<(), Error> {
        if settings == self.settings.get() {
            return Ok(());
        }
        self.kv.access(|store, scratch| {
            store.store(LIGHT_SETTINGS_KEY, &settings.encode(), scratch)
        })?;
        let old = self.settings.replace(settings);
        if old.enabled != settings.enabled {
            info!("Matter LED enabled: {}", settings.enabled);
            self.changed.signal(());
        }
        Ok(())
    }

    pub async fn run<L: SmartLedsWrite<Color = RGB8>>(&self, led: &mut L)
    where
        L::Error: core::fmt::Debug,
    {
        let mut sequence = BlinkSequence::default();
        loop {
            self.changed.reset();
            let enabled = self.on_off();
            let (name, [r, g, b]) = sequence.next_frame(enabled);
            led.write([RGB8::new(r, g, b)])
                .expect("Failed to update onboard LED");
            info!("LED: {}", name);
            if enabled {
                // A command wakes this task immediately instead of waiting for the 500 ms timer.
                select(self.changed.wait(), Timer::after_millis(HALF_PERIOD_MS)).await;
            } else {
                self.changed.wait().await;
            }
        }
    }
}

impl<K: KvBlobStoreAccess> OnOffHooks for BoardLight<'_, K> {
    const CLUSTER: Cluster<'static> = LIGHT_CLUSTER;

    fn on_off(&self) -> bool {
        self.settings.get().enabled
    }

    fn set_on_off(&self, enabled: bool) {
        self.save(LightSettings {
            enabled,
            ..self.settings.get()
        })
        .expect("Failed to persist Matter On/Off state");
    }

    fn start_up_on_off(&self) -> Nullable<StartUpOnOffEnum> {
        match self.settings.get().startup {
            Some(0) => Nullable::some(StartUpOnOffEnum::Off),
            Some(1) => Nullable::some(StartUpOnOffEnum::On),
            Some(2) => Nullable::some(StartUpOnOffEnum::Toggle),
            Some(_) => panic!("Invalid stored startup mode"),
            None => Nullable::none(),
        }
    }

    fn set_start_up_on_off(&self, value: Nullable<StartUpOnOffEnum>) -> Result<(), Error> {
        self.save(LightSettings {
            startup: value.into_option().map(|value| value as u8),
            ..self.settings.get()
        })
    }

    async fn handle_off_with_effect(&self, _effect: EffectVariantEnum) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs_matter_embassy::matter::persist::KvBlobStore;
    use std::{cell::RefCell, collections::BTreeMap, vec::Vec};

    #[derive(Default)]
    struct MemoryStore {
        records: BTreeMap<u16, Vec<u8>>,
        fail_store: bool,
    }

    impl KvBlobStore for MemoryStore {
        fn load<'a>(&mut self, key: u16, buf: &'a mut [u8]) -> Result<Option<&'a [u8]>, Error> {
            let Some(bytes) = self.records.get(&key) else {
                return Ok(None);
            };
            buf[..bytes.len()].copy_from_slice(bytes);
            Ok(Some(&buf[..bytes.len()]))
        }

        fn store(&mut self, key: u16, data: &[u8], _: &mut [u8]) -> Result<(), Error> {
            if self.fail_store {
                return Err(ErrorCode::Invalid.into());
            }
            self.records.insert(key, data.to_vec());
            Ok(())
        }

        fn remove(&mut self, key: u16, _: &mut [u8]) -> Result<(), Error> {
            self.records.remove(&key);
            Ok(())
        }
    }

    #[derive(Default)]
    struct MemoryAccess(RefCell<MemoryStore>);

    impl KvBlobStoreAccess for MemoryAccess {
        fn access<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&mut dyn KvBlobStore, &mut [u8]) -> R,
        {
            f(&mut *self.0.borrow_mut(), &mut [0; 4096])
        }
    }

    #[test]
    fn thread_record_survives_led_initialization_commands_and_reboot() {
        let kv = MemoryAccess::default();
        let thread_record = [8, 0, 42, 43, 44, 45];
        let fabric_record = [10, 20, 30];
        kv.0.borrow_mut()
            .records
            .insert(THREAD_SRP_SETTINGS_KEY, thread_record.to_vec());
        kv.0.borrow_mut().records.insert(1, fabric_record.to_vec());

        {
            let light = BoardLight::load(&kv).unwrap();
            assert!(!light.on_off());
            light.set_on_off(true);
            light
                .set_start_up_on_off(Nullable::some(StartUpOnOffEnum::Toggle))
                .unwrap();
        }

        let restored = BoardLight::load(&kv).unwrap();
        assert_eq!(
            restored.settings.get(),
            LightSettings {
                enabled: true,
                startup: Some(2),
            }
        );
        let store = kv.0.borrow();
        assert_eq!(store.records[&THREAD_SRP_SETTINGS_KEY], thread_record);
        assert_eq!(store.records[&1], fabric_record);
    }

    #[test]
    fn migrates_legacy_led_record_without_erasing_fabric() {
        let kv = MemoryAccess::default();
        let settings = LightSettings {
            enabled: true,
            startup: Some(1),
        };
        kv.0.borrow_mut()
            .records
            .insert(THREAD_SRP_SETTINGS_KEY, settings.encode().to_vec());
        kv.0.borrow_mut().records.insert(1, [10, 20, 30].to_vec());

        assert_eq!(BoardLight::load(&kv).unwrap().settings.get(), settings);
        assert_eq!(BoardLight::load(&kv).unwrap().settings.get(), settings);
        let store = kv.0.borrow();
        assert!(!store.records.contains_key(&THREAD_SRP_SETTINGS_KEY));
        assert_eq!(store.records[&LIGHT_SETTINGS_KEY], settings.encode());
        assert_eq!(store.records[&1], [10, 20, 30]);
    }

    #[test]
    fn interrupted_migration_keeps_newer_settings() {
        let kv = MemoryAccess::default();
        kv.0.borrow_mut()
            .records
            .insert(THREAD_SRP_SETTINGS_KEY, [1, 1, 255].to_vec());
        kv.0.borrow_mut()
            .records
            .insert(LIGHT_SETTINGS_KEY, [1, 0, 255].to_vec());

        assert!(!BoardLight::load(&kv).unwrap().on_off());
        assert!(!kv.0.borrow().records.contains_key(&THREAD_SRP_SETTINGS_KEY));
    }

    #[test]
    fn failed_migration_write_preserves_legacy_record() {
        let kv = MemoryAccess::default();
        kv.0.borrow_mut()
            .records
            .insert(THREAD_SRP_SETTINGS_KEY, [1, 1, 255].to_vec());
        kv.0.borrow_mut().fail_store = true;

        assert!(BoardLight::load(&kv).is_err());
        assert_eq!(kv.0.borrow().records[&THREAD_SRP_SETTINGS_KEY], [1, 1, 255]);
    }

    #[test]
    fn invalid_new_settings_are_reported_without_overwriting_records() {
        let kv = MemoryAccess::default();
        let records = BTreeMap::from([
            (LIGHT_SETTINGS_KEY, [2, 0, 255].to_vec()),
            (THREAD_SRP_SETTINGS_KEY, [8, 0, 42, 43].to_vec()),
        ]);
        kv.0.borrow_mut().records = records.clone();

        assert!(BoardLight::load(&kv).is_err());
        assert_eq!(kv.0.borrow().records, records);
    }
}
