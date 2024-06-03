# Mutiny

Peer-to-peer web applications runtime.

## Structure

* mutiny - user-friendly CLI interface to mutinyd
* mutinyd - long-running process to manage networking, persistence, data sync
* mutiny-app - serves an application and provides HTTP API
* lib/ - shared code for the above applications

## Usage

First, run the daemon:

```
cd mutinyd
cargo run
```

Then, run mutiny to check communication between daemon and CLI:

```
./mutiny/mutiny
```

## Examples

### Ping

Demonstrates communication between frontend, mutiny-app backend, and mutinyd daemon.

First, start the mutinyd daemon:

```
cd mutinyd
cargo run
```

Then, serve the app:

```
./mutiny-app/mutiny-app examples/ping
```

And open the displayed URL in your browser. You should see your local peer ID.
