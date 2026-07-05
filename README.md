# rp2040_temp_fan_ctrl

Fan Controller based around an RP2040 micro controller, built on the
[Embassy](https://embassy.dev/) async embedded framework.

## How it works

The RP2040 enumerates as a USB CDC-ACM serial device (`USB-serial fan
controller`). A host (e.g. a Raspberry Pi cluster monitor) connects over
that serial port and periodically sends the current maximum rack
temperature in °C as an ASCII decimal string (e.g. `47\n`).

Fan control logic:

- If any reported temp exceeds **45 °C**, the fans turn on.
- Once on, the fans stay on for a minimum run time before being
  re-evaluated, to avoid rapid on/off cycling.
- **Failsafe**: if the host hasn't sent an update in 60 seconds, the
  fans are forced on, regardless of last known temperature.
- On every fan state change, the board writes `FAN_ON` / `FAN_OFF` back
  over the serial port.

> **Work in progress**: the current code drives the onboard LED (GPIO
> 25) as a stand-in for the fan relay output while the pinout is being
> finalized, and the fan run duration is shortened for bench testing.
> See `src/main.rs` for the relevant constants (`TARGET_TEMP`,
> `fan_run_duration`) before flashing for production use.

The relay used is a JZC-11F; energizing its coil closes the relay and
powers the fans through the normally-open contact. See the earlier
schematic (`fan_ctrl.fzz` / `fan_ctrl_with_led.fzz`) for wiring
details — a flyback diode/LED across the coil dissipates current when
it de-energizes.

# To Build project

```bash
$ cargo build --release
```

or, to also produce the flashable `.uf2` file:

```bash
$ make build
```

# To Flash the RP2040

Hold **BOOTSEL** button and connect to USB

Once mounted, run:

```bash
$ cargo run --release
```

_Note_: This requires the elf2uf2-rs cargo plugin. Install this using:

```bash
$ cargo install elf2uf2-rs
```

See [JoNil/elf2uf2-rs](https://github.com/JoNil/elf2uf2-rs) for code and details.
