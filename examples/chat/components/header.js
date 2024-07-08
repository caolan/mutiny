import {watch} from "../lib/signaller.js";
import {local_peer_id} from "../state.js";

export default class ChatHeader extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        const shadow = this.attachShadow({mode: "open"});
        shadow.innerHTML = `
            <link rel="stylesheet" href="style.css">
            <header>
                <h1>Chat Example</h1>
                <span id="local-peer-id"></span>
            </header>
        `;
        const setText = () => {
            const span = shadow.getElementById('local-peer-id');
            span.textContent = `You: ${local_peer_id.value}`;
        };
        setText();
        this.stop = watch([local_peer_id], setText);
    }

    disconnectedCallback() {
        this.stop();
    }
}

customElements.define("chat-header", ChatHeader);
