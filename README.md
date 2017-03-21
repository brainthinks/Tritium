# Tritium
Tritium is a Halo: Combat Evolved cache file rebuilder written in Rust. It's capable of disassociating tags from map files to make them much easier to manage, then reassembling the map file. It also provides functions for removing and inserting tags, as well.

To use Tritium in your project, I recommend either cloning/forking it and specifying the path in your project's cargo.toml file, or if you want to use this repository directly, add something like this to your cargo.toml file:
```toml
[dependencies]
tritium = {git = "https://github.com/Halogen002/Tritium", branch = "0.6.0"}
```
