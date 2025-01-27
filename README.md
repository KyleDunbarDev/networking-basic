# Developer documentation
Below is documentation to help you use and understand this networking starter piece. <br/>
This is not a swiss army knife, but a chassis for you to build upon.<br/><br/>
Client (client/game & bin/client) is an example for a game and can be fully swapped out for any executable of your choosing. <br/><br/>
For client - server interoperability, send serialised messages to the server that be can deserialised into ClientMessage types.
Serlialisation in the server is in JSON but this can be swapped easily for serailisation of your choice.

# Stability

```mermaid
---
title: Dependency Model
---
graph TD;

  Server/Network --> Common/Messages
  Server/Network --> Common/Errors
  Server/Network --> Common/Types
  Server/Game --> Common/Messages
  Server/Game --> Common/Errors
  Server/Game --> Common/Types
  Common/Messages --> Common/Types
  Client/Game --> Common/Messages
  Client/Game --> Common/Errors
  Client/Game --> Common/Types
  bin/Server --> Common/Messages
  bin/Server --> Common/Errors
  bin/Server --> Common/Types
  bin/Client --> Client/Game
  bin/Client --> Common/Messages
  bin/Client --> Common/Errors
  bin/Client --> Common/Types
  bin/Client --> lib
  

```
