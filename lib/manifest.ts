import { assert, AssertionError } from "@std/assert";

export type Manifest = {
    id: string,
    version: string,
};

export function parseManifest(bytes: Uint8Array): Manifest {
    const text = new TextDecoder().decode(bytes);
    const data = JSON.parse(text);
    assert(typeof(data.id) === "string", "missing or invalid 'id' property");
    assert(typeof(data.version) === "string", "missing or invalid 'version' property");
    return data;
}

export async function readManifest(filename: string): Promise<Manifest> {
    const bytes = await Deno.readFile(filename);
    try {
        return parseManifest(bytes);
    } catch (e) {
        if (e instanceof AssertionError) {
            throw new Error(`Failed to read ${filename}: ${e.message}`);
        }
        throw e;
    }
}
