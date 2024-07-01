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
./mutiny-app/mutiny-app ping examples/ping
```

And open the displayed URL in your browser. You should see your local peer
ID at the top of the page and a (potentially empty) list of discovered
peers below. Repeating this process on another machine on your local
network will hopefully add a discovered peer to the list.

### Chat

Demonstrates peer-to-peer message delivery on the local network.

First, start the mutinyd daemon:

```
cd mutinyd
cargo run
```

Then, serve the app:

```
./mutiny-app/mutiny-app chat examples/chat
```

And open the displayed URL in your browser. Repeat this process on another
machine on your local network and it will hopefully add a discovered
peer to the list on the left. Click on the peer to type messages to
each another.

## Tests

### Integration

Integration tests can be found in the top `tests` directory - these
exercise multiple programs and test the interation between them.

```
cd tests
deno test -A
```

### mutinyd

Unit tests for the mutiny daemon:

```
cd mutinyd
cargo test
```

### mutiny-app

Unit tests for the mutiny-app server:

```
cd mutiny-app
deno test -A
```
