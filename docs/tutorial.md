# Mutiny App Tutorial

Authoring an app for Mutiny requires only static HTML/JS/CSS, and a familiarity
with the Mutiny app API.

[Mutiny app API documentation](https://github.com/caolan/mutiny/blob/main/docs/api.md)

## Installation

```
git clone https://github.com/caolan/mutiny.git
cd mutiny
```

## Starting the daemon

The `mutinyd` daemon acts as your agent on the peer-to-peer network. It
must be running before your can serve mutiny applications.

```
cd mutinyd
cargo run
```

## Creating an application to list discovered peers

Because Mutiny applications are simple HTML/JS/CSS, you don't need any
special tooling to create one. You're also free to use any framework or
no framework at all.

Let's start a new application by creating a directory to contain the
HTML/JS/CSS:

```
mkdir peers-app
cd peers-app
```

We'll add a basic layout to `index.html`:

```html
<!DOCTYPE html>
<html>
    <head>
        <meta charset="utf-8" />
        <title>Peers</title>
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
    </head>
    <body>
        <h1>Peers</h1>
        <ul id="peers"></ul>
    </body>
</html>
```

## Serving the app

You can serve this application using `mutiny-app`:

```
./mutiny-app/mutiny-app mylabel ./peers-app
```

This tells `mutinyd` to register a new application with label `mylabel` and
then serves it using your static assets in `./peers-app`. You should see
something like the following in your console:

```
Connecting to /run/user/1000/mutiny/mutinyd.socket
Application:
  uuid: 5eb1bbae-6a19-4427-bf96-3437e796b408
  label: mylabel

Serving ./peers-app:
  http://127.0.0.1:33337/
```

Opening the URL printed to your console should display the index page
we just added.

## Listing discovered peers

Now let's actually use the Mutiny app API to display discovered peers.
Update `index.html` as follows:

```html
<!DOCTYPE html>
<html>
    <head>
        <meta charset="utf-8" />
        <title>Peers</title>
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
    </head>
    <body>
        <h1>Peers</h1>
        <ul id="peers"></ul>
        <script type="module">
            // Request discovered peers and display on page
            const ul = document.getElementById('peers');
            const res = await fetch("/_api/v1/peers");
            const peers = await res.json();
            for (const peer of peers) {
                const li = document.createElement('li');
                li.textContent = peer;
                ul.appendChild(li);
            }
        </script>
    </body>
</html>
```

Of course, there are probably no other peers on your local network to
discover yet. Let's start another instance of `mutinyd` on our local
machine.

```
cd mutinyd
cargo run -- --socket /tmp/mutiny.sock --data /tmp/mutiny-data
```

Using custom `--socket` and `--data` flags, we can start multiple
instances of `mutinyd` to test with. After starting, you should
see your two running instances of `mutinyd` discover one another
in their console output:

```
[caolan@caolan-desktop mutinyd]$ cargo run
    Finished dev [unoptimized + debuginfo] target(s) in 0.13s
     Running `target/debug/mutinyd`
Reading identity "/home/caolan/.local/share/mutiny/identity.key"
  Local peer ID: 12D3KooWBiievVgzqDYaUTvM4dgVKxW4X26FyQ3YZ3FrxdSANUP9
New listener: /ip4/127.0.0.1/tcp/34077
New listener: /ip4/192.168.2.54/tcp/34077
mDNS discovered a new peer: 12D3KooWDRr3g9ToQi33hDkruPMh5JvTLsyhiEeX22R6bkwW2UNc
```

If your refresh your application in the browser, you should see it now
lists the other peer.

## Subscribing to events

Instead of refreshing the page, you can use the EventSource API to be
notified when a new peer is discovered.

```html
<!DOCTYPE html>
<html>
    <head>
        <meta charset="utf-8" />
        <title>Peers</title>
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
    </head>
    <body>
        <h1>Peers</h1>
        <ul id="peers"></ul>
        <script type="module">
            const ul = document.getElementById('peers');

            // Subscribe to peer events and append newly discovered
            // peers to list
            const source = new EventSource("/_api/v1/peers/events");
            source.addEventListener("PeerDiscovered", event => {
                const li = document.createElement('li');
                li.textContent = event.data;
                ul.appendChild(li);
            });

            // Request discovered peers and display on page
            const res = await fetch("/_api/v1/peers");
            const peers = await res.json();
            for (const peer of peers) {
                const li = document.createElement('li');
                li.textContent = peer;
                ul.appendChild(li);
            }
        </script>
    </body>
</html>
```

Refresh the page once more to get the latest code, then spawn another
instance of `mutinyd` to see it appear automatically in your browser
window.

```
cd mutinyd
cargo run -- --socket /tmp/mutiny2.sock --data /tmp/mutiny2-data
```

This pattern of subscribing to events using `EventSource()` then
requesting initial data using `fetch()` is used across the Mutiny app
API.

To develop this application further, try out the Announce and Messaging
APIs in the [documentation](https://github.com/caolan/mutiny/blob/main/docs/api.md).
