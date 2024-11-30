# Museum Assassin

This repository contains the source for "Museum Assassin," my submission to the 2024 Game Off jam. You can find the full submission on [itch.io](https://rjley.itch.io/museum-assassin).

## Building

Unfortunately, I cannot include the `assets/` folder in this release without violating some asset pack licenses. Because the game copies assets into the output binary at build time, this means that you cannot build Museum Assassin directly from this repo. If you do have the `assets/` folder, then you will also have to clone my fork of `macroquad/` and place it at `../macroquad` before running `cargo build`, and things should work from there.