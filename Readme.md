# Crazyflie Commander

Controlling a [Crazyflie][] nano-drone from a Rust terminal UI.

![mission](media/mission.gif)

## Crates

- [`ratatea`](ratatea): an [Elm Architecture][tea] runtime for [ratatui][] TUIs.
- [`drone-control`](drone-control): library for flying missions on a Crazyflie over the radio link.
- [`drone-commander`](drone-commander): the terminal UI, built on the two crates above.

[Crazyflie]: https://www.bitcraze.io/products/crazyflie-2-1-plus/

[tea]: https://guide.elm-lang.org/architecture/

[ratatui]: https://ratatui.rs