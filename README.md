# Developer documentation
Following is the UML and design documentation for the development of MPFree. If you wish to contribute, please start here

# Stability

```mermaid
---
title: Dependency Model
---
classDiagram

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
  bin/Client --> Server/Game
  

```
