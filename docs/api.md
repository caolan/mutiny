# Mutiny App API

This document describes the API provided to Mutiny applications running
in the browser. Access it using `fetch()` or `EventSource()` where
appropriate.

## Application metadata

An application is referenced canonically by it's UUID, but also
locally using a human-friendly label. If you serve the chat app using
`mutiny serve chat examples/chat`, you are serving the `examples/chat`
directory with the app label `chat`. You'll see the app's UUID printed
after starting it.

Use the following API to request the current app's label and UUID in
the browser:

```
GET /_api/v1/application

Expected response:
{uuid: string, label: string}
```

## Peers

A peer is another `mutinyd` instance on the peer-to-peer network.

You can discover the local `mutinyd`'s peer ID using:

```
GET /_api/v1/local_peer_id

Expected response:
string
```

And list discovered remote peer IDs using:

```
GET /_api/v1/peers

Expected response:
string[]
```

Note that discovery is currently only enabled for machines on your
local network.

You can also subscribe to live peer discovery/expiry events in the
browser using `EventSource()`:

```
GET /_api/v1/peers/events

Expected response:

event: PeerDiscovered
data: string

event: PeerExpired
data: string

...
```

Example use in the browser:

```
const peers = new EventSource("/_api/v1/peers/events");

peers.addEventListener("PeerDiscovered", id => {
    console.log("Peer discovered", id);
});

peers.addEventListener("PeerExpired", id => {
    console.log("Peer expired", id);
});
```

## Announcements

Announcements are how Mutiny apps discover one another. Each app may
have one active announcement at any given time - updating an app's
announcement data will replace the its previous value.

Announcments are readable by all apps at the destination peer.

To read your received announcements:

```
GET /_api/v1/announcements/inbox

Expected response:
{
    peer: string, 
    app_uuid: string,
    data: <json value>,
}[]
```

The data associated with an app announcement is a free-form JSON
field. For example, the chat app uses it to announce the nickname of
the user to other chat instances it finds.

You can subscribe to new announcements using `EventSource()`:

```
GET /_api/v1/announcements/inbox/events

Expected response:

event: AppAnnouncement
data: {
    peer: string, 
    app_uuid: string,
    data: <json value>,
}

event: AppAnnouncement
data: {
    peer: string, 
    app_uuid: string,
    data: <json value>,
}

...
```

Example use in the browser:

```
const announcements = new EventSource("/_api/v1/announcements/events");

announcements.addEventListener("AppAnnouncement", event => {
    console.log("App announcement received", event);
});
```

To send an app announcement to a peer (including the local peer, if you
want to make this app discoverable to other local apps):

```
POST /_api/v1/announcements/outbox

Request body:
{peer: string, data: string}

Expected response:
{success: true}
```

## Messages

Once two apps have discovered one another they can exchange messages
addressed using the peer ID and the app UUID.

To read your received messages:

```
GET /_api/v1/messages/inbox

Expected response:
{
    id: number,
    peer: string,
    uuid: string,
    message: string,
}[]
```

You can also subscribe to incoming message events in the browser using
`EventSource()`:

```
GET /_api/v1/messages/inbox/events

Expected response:

event: Message
data: {
    id: number,
    peer: string,
    uuid: string,
    message: string,
}

event: Message
data: {
    id: number,
    peer: string,
    uuid: string,
    message: string,
}

...
```

To send a message (including to other apps at the local peer):

```
POST /_api/v1/messages/outbox

Request body:
{peer: string, app_uuid: string, message: string}

Expected response:
{success: true}
```

It's often convenient to encode the message body string using JSON.
