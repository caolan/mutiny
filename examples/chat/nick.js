import {local_peer_id, nick} from "./state.js";

export function askNick() {
    const default_value = nick.value || local_peer_id.value || "";
    const value = prompt("Enter nick:", default_value);
    if (value) {
        nick.value = value;
    }
}
