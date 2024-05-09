import { readAll } from "jsr:@std/io/read-all";

const socket_path = Deno.args[0] || './fed.socket';

console.log(`Connecting to ${socket_path}`);
const conn = await Deno.connect({
    transport: 'unix',
    path: socket_path,
});

const bytes = await readAll(conn);
const text = new TextDecoder().decode(bytes);
console.log(text);

conn.close();
