use core::cell::Cell;

use defmt::info;
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
        let settings =
            kv.access(
                |store, scratch| match store.load(VENDOR_KEYS_START, scratch)? {
                    Some(bytes) => {
                        info!("Loaded persistent LED settings");
                        LightSettings::decode(bytes).ok_or_else(|| Error::from(ErrorCode::Invalid))
                    }
                    None => {
                        info!("Initializing persistent LED settings: off");
                        let settings = LightSettings::default();
                        store.store(VENDOR_KEYS_START, &settings.encode(), scratch)?;
                        Ok(settings)
                    }
                },
            )?;
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
        self.kv
            .access(|store, scratch| store.store(VENDOR_KEYS_START, &settings.encode(), scratch))?;
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
