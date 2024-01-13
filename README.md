# Minecraft Beta 1.7.3
A work-in-progress Minecraft beta 1.7.3 server made in Rust. This project split the server
crate from data structure and logic crate, the latter is made to be used be developers.

1. [Logic crate](#logic-crate)
2. [Server crate](#server-crate)
3. [Contributing](#contributing)
4. [Roadmap](#roadmap)

## Logic crate

[![Crates.io Total Downloads](https://img.shields.io/crates/d/mc173?style=flat-square)](https://crates.io/crates/mc173)

The logic crate [mc173](/mc173/) provides the core data structures such as worlds, chunks 
and entities, but also the behaviors for blocks, items and entities. It also provides a
lot of utilities related to Minecraft.

## Server crate

[![Crates.io Total Downloads](https://img.shields.io/crates/d/mc173-server?style=flat-square)](https://crates.io/crates/mc173-server)

The server crate [mc173-server](/mc173-server/) is an implementation of the *Notchian* 
server protocol, it is built on top of the logic crate and has threaded networking, it 
also defines protocol structures.

## Contributing

If you're willing to contribute or fork this code, this sections presents the different
tools that can be used to understand the *Notchian* implementation of Minecraft beta 
1.7.3 and how to implement it into Rust.

The most important tool is [RetroMCP], which is a modern remake of *MCP* (one of the most important software in Minecraft's modding history). It can be used to automatically
decompile and deobfuscate the original archive of Minecraft beta 1.7.3. It can also be
used to recompile and reobfuscate the game and then run it, which can be useful to add
debugging code, but fortunately it's rare to get to that point. You can read the project's
README, it is really well designed and its CLI is intuitive, you just have to choose the
b1.7.3 version for both client and server.

Choosing both client and server is really important as these two have slightly different
source codes. For example, you have to choose the client or server source code depending
on which side of the network protocol you want to understand.

The next step is just to explore the source code, and try to understand how it works! This
can be quite challenging sometimes due to the object oriented nature of it, so you should
also use a IDE or text editor that support the Java langage and a few important features
such as *goto definition* and *class hierarchy* (VSCode, IDEA, Eclipse...).

Use the following [roadmap](#roadmap) either to understand how the completed components
have been adapted from Java to Rust, or if you want to contribute and add a feature.
The Rust code is also documented as most as possible, so please read the doc comments
to really understand how to contribute to the documented code. If you think that the
roadmap is incomplete, you can add items as needed.

A tool that can also be useful is a Minecraft CLI launcher that I *(Théo Rozier)* made,
it's called [PortableMC] and it has really good support for b1.7.3 and the game starts
really fast compared to the Mojang launcher. It also fixes in-game skin and some other
legacy-related issues.

[RetroMCP]: https://github.com/MCPHackers/RetroMCP-Java
[PortableMC]: https://github.com/mindstorm38/portablemc

## Roadmap
There is a lot of work to be done in order to provide a fully functional server on 
parity with the *Notchian* server, in order to properly achieve this work, the following
roadmap summarize implemented and missing components and in which order we should work
on them. The priority of each feature is defined by its order in the list.

- [x] World and chunk data structures
- [ ] World serialization
    - [x] Chunk data
    - [x] Block entity data
    - [x] Entity data
    - [ ] Level data
- [ ] Blocks
    - [x] Definitions
    - [x] Item drop
    - [x] Tick scheduling
    - [x] Placing
    - [x] Breaking
    - [ ] Redstone (partial)
    - [ ] Clicking (partial)
- [ ] Items
    - [x] Definitions
    - [x] Inventory data structure
    - [x] Crafting
        - [x] Definitions
        - [x] Tracker
    - [ ] Use/place behaviors
    - [x] Break behaviors
- [ ] Entities
    - [x] Entity data structures
    - [ ] Entity behaviors (80%)
- [ ] Server
    - [x] Protocol
    - [x] Network threading
    - [x] Block breaking
        - [x] Long block breaking
        - [x] Instant block breaking
        - [x] Block breaking duration check
    - [x] Players inventory is stored server-side
    - [ ] Players can be linked to any entity type
    - [ ] Worlds serialization
        - [ ] Non-persistent player entities
        - [ ] Player entities saved appart
    - [x] Player window
        - [x] Left and right click support
        - [x] Player inventory crafting grid
        - [x] Crafting table
        - [x] Chest
        - [x] Furnace
        - [x] Dispenser
        - [x] Shift-click on items
    - [x] Entity tracking
        - [x] Client-side spawn
- [x] Lighting engine
- [x] World generation