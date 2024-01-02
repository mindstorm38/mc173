# Minecraft Beta 1.7.3
A work-in-progress Minecraft beta 1.7.3 server made in Rust. This project split the server
crate from data structure and logic crate, the latter is made to be used be developers.

## Logic
The logic crate [mc173](/mc173/) provides the core data structures such as worlds, chunks 
and entities, but also the behaviors for blocks, items and entities. It also provides a
lot of utilities related to Minecraft.

## Server
The server crate [mc173-server](/mc173-server/) is an implementation of the *Notchian* 
server protocol, it is built on top of the logic crate and has threaded networking, it 
also defines protocol structures.

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
    - [ ] Player window
        - [x] Left and right click support
        - [x] Player inventory crafting grid
        - [x] Crafting table
        - [x] Chest
        - [x] Furnace
        - [x] Dispenser
        - [ ] Shift-click on items
    - [x] Entity tracking
        - [x] Client-side spawn
- [x] Lighting engine
- [x] World generation
