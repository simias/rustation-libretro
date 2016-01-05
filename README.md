# Libretro frontend Rustation

This is an implementation of the [libretro](http://www.libretro.com/)
API for the [Rustation PlayStation
emulator](https://github.com/simias/rustation).

## Build

You'll need [Rust and its package manager
Cargo](https://www.rust-lang.org/). Cargo will take care of
downloading the various dependencies used by this crate.

The Rustation source code is a git submodule of this repository so
you'll want to clone it with:

```
git clone --recursive https://github.com/simias/rustation-libretro.git
```

Then you should be able to build the libretro core from the
`rustation-libretro` directory using:

```
cargo build --release
```

The core will be in the `target/release/` directory. It should be
named `librustation_retro.so` on unix.

To run the core you'll need a [libretro
frontend](http://wiki.libretro.com/index.php?title=Frontends), I'm
mostly using
[RetroArch](http://wiki.libretro.com/index.php?title=RetroArch) but
please do submit an issue if there's a compatibility problem with an
other frontend.

## BIOS files

You'll need to put a PlayStation BIOS in the "system directory" of the
frontend you're using.

Rustation-libretro will search for files in this directory until it
finds a valid BIOS for the game's region. Consult your frontend's
documentation to figure out where it puts the system
directory. Alternatively check the logs after loading a game to see
where Rustation-libretro is looking for the BIOS, you should see a
message similar to:

```
Looking for a BIOS for region <Region> in "/some/directory"
```

You'll need different BIOSes for the japanese, north-american, and
european regions since PlayStation games are region-locked. You can
just put all the BIOS files in the system directory and let
Rustation-libretro figure out which one to use for the game you're
using.

If for some reason Rustation-libretro doesn't seem to pick up on your
BIOS file check the logs to see why. The BIOS must match one of the
entries in Rustation's internal database (see `src/bios/db.rs` in
Rustation's source code) otherwise it'll be ignored. If the BIOS
you're using is not part of the database chances are it's a bad dump.
