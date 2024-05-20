# FuckEvil

Peer-to-peer web applications runtime.

## Structure

* fe - user-friendly CLI interface to fed
* fed - long-running process to manage networking, persistence, data sync
* fes - serves an application and provides HTTP API
* lib/ - shared code for the above applications

## Usage

First, run the daemon:

```
cd fed
cargo run
```

Then, run fe to check communication between daemon and CLI:

```
cd fe
./fe ../fed/fed.socket
```
