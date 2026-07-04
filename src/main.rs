#![no_std]
#![no_main]

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    gpio::{Level, Output},
    peripherals::USB,
    usb::{Driver, Instance, InterruptHandler},
};
use embassy_time::{Duration, Instant, Timer};
use embassy_usb::{
    UsbDevice,
    class::cdc_acm::{CdcAcmClass, State},
    driver::EndpointError,
};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting...");

    let p = embassy_rp::init(Default::default());

    let mut led = Output::new(p.PIN_25, Level::Low);

    // Create the driver, from the HAL.
    let driver = Driver::new(p.USB, Irqs);

    // Create embassy-usb Config
    let config = {
        let mut config = embassy_usb::Config::new(0xffff, 0xffff);
        config.manufacturer = Some("JPLabs");
        config.product = Some("USB-serial fan controller");
        config.serial_number = Some("DEADBEEF");
        config.max_power = 100;
        config.max_packet_size_0 = 64;
        config
    };

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

        embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            &mut [], // no msos descriptors
            CONTROL_BUF.init([0; 64]),
        )
    };

    // Create classes on the builder.
    let mut class = {
        static STATE: StaticCell<State> = StaticCell::new();
        let state = STATE.init(State::new());
        CdcAcmClass::new(&mut builder, state, 64)
    };

    // Build the builder.
    let usb = builder.build();

    // Run the USB device.
    spawner.spawn(unwrap!(usb_task(usb)));

    // Do stuff with the class!
    loop {
        class.wait_connection().await;
        info!("Connected");
        let _ = class.write_packet(b"CONNECTED\n").await;
        let _ = echo(&mut class, &mut led).await;
        info!("Disconnected");
    }
}

type MyUsbDriver = Driver<'static, USB>;
type MyUsbDevice = UsbDevice<'static, MyUsbDriver>;

#[embassy_executor::task]
async fn usb_task(mut usb: MyUsbDevice) -> ! {
    usb.run().await
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

static TARGET_TEMP: u8 = 45; // Turn on if any Pi goes over 45°C

async fn echo<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
    led: &mut Output<'d>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 64];
    let mut last_update = Instant::now();
    let mut current_max_temp = 0u8;
    let mut fans_active = false;
    let mut fan_start_time = Instant::now();
    // let fan_run_duration = Duration::from_secs(5 * 60); // X = 5 minutes
    let fan_run_duration = Duration::from_secs(5); // X = 5 seconds

    loop {
        let n = class.read_packet(&mut buf).await?;
        let data = &buf[..n];

        // Each packet is the rack temp in degrees Celsius, as an ASCII decimal string.
        if let Ok(temp) = core::str::from_utf8(data)
            .unwrap_or("")
            .trim()
            .parse::<u8>()
        {
            current_max_temp = temp;
            last_update = Instant::now();
        }

        let was_active = fans_active;

        // --- Safety Timeout Check ---
        if Instant::now().duration_since(last_update) > Duration::from_secs(60) {
            // Master Pi hasn't talked to us in 60 seconds! Failsafe mode.
            led.set_high();
            fans_active = true;
        } else if current_max_temp > TARGET_TEMP && !fans_active {
            // --- Normal Fan Logic ---
            led.set_high();
            fans_active = true;
            fan_start_time = Instant::now();
        } else if fans_active && Instant::now().duration_since(fan_start_time) >= fan_run_duration {
            // X minutes have elapsed, check if cool enough to turn off
            if current_max_temp <= TARGET_TEMP {
                led.set_low();
                fans_active = false;
            } else {
                // Still too hot, reset the timer for another X minutes
                fan_start_time = Instant::now();
            }
        }

        if fans_active != was_active {
            let event: &[u8] = if fans_active {
                b"FAN_ON\n"
            } else {
                b"FAN_OFF\n"
            };
            class.write_packet(event).await?;
        }

        Timer::after(Duration::from_millis(1000)).await;
    }
}
