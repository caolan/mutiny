import { assert, assertEquals } from "@std/assert";
import { exists } from "@std/fs/exists";
import { join } from "@std/path/join";

function timeout(ms: number): Promise<null> {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function waitForSocket(path: string) {
    while (!(await exists(path))) await timeout(10); 
}

Deno.test("Connect CLI to mutinyd and check it prints local peer ID", async () => {
    const run_dir = await Deno.makeTempDir();
    const data_dir = join(import.meta.dirname as string, "./data/cli_local_peer_id");
    const socket_path = join(run_dir, "mutiny.socket");

    const mutinyd = new Deno.Command("cargo", {
        args: ["run", "--", "--socket", socket_path, "--data", data_dir],
        cwd: join(import.meta.dirname as string, "../mutinyd"), 
        stdout: "null",
        stderr: "null",
    }).spawn();

    await waitForSocket(socket_path);

    const {code, stdout } = await new Deno.Command("bash", {
        args: ["./mutiny/mutiny", socket_path],
        cwd: join(import.meta.dirname as string, ".."), 
    }).output();

    // Check mutiny CLI connected and reported local peer ID
    const txt = new TextDecoder().decode(stdout);
    assert(txt.includes("12D3KooWGTq2hgqpzUpk8Z1TGENuxkxCsZjxBtD7e8kWAUawcygo"));

    mutinyd.kill("SIGINT");
    await mutinyd.status;
    assertEquals(code, 0);
});
