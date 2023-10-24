# Minecraft Beta 1.7.3
A work-in-progress Minecraft beta 1.7.3 server made in Rust. This project split the server
crate from data structure and logic crate, the latter is made to be used be developers.

## Logic
The logic crate is available at [mc173/](/mc173/) and provides the core data structures
such as worlds, chunks and entities, but also the behaviors for blocks, items and 
entities.

## Server
The server crate is available at [mc173-server/](/mc173-server/), it's an implementation 
of the *Notchian* server protocol, it is built on top of the logic crate and has threaded 
networking, it also defines protocol structures.

## Roadmap
There is a lot of work to be done in order to provide a fully functional server on 
parity with the *Notchian* server, in order to properly achieve this work, the following
roadmap summarize implemented and missing components and in which order we should work
on them. The priority of each feature is defined by its order in the list.

- [x] World and chunk data structures
- [ ] Blocks
    - [x] Definitions
    - [x] Drop behaviors
    - [ ] Scheduled ticks
    - [ ] Break behaviors
    - [ ] Interact behaviors
    - [ ] Redstone
- [ ] Items
    - [x] Definitions
    - [x] Inventory data structure
    - [x] Crafting
        - [x] Definitions
        - [x] Tracker
    - [ ] Use/place behaviors
    - [ ] Break behaviors
- [ ] Entities
    - [ ] Entity data structures (partial)
    - [ ] Entity behaviors (partial)
- [ ] Server
    - [x] Protocol
    - [x] Network threading
    - [ ] Player window
        - [x] Left and right click support
        - [x] Player inventory crafting grid
        - [ ] Crafting table
        - [ ] Chest
        - [ ] Furnace
        - [ ] Shift-click on items
    - [ ] Block breaking
        - [x] Long block breaking
        - [ ] Instant block breaking
        - [ ] Block breaking duration check
    - [ ] Entity tracking
        - [ ] Client-side spawn (partial) 
- [ ] World generation
- [ ] Lighting engine
