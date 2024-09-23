// Either read until dest ArrayBuffer is full or reject with error.
export async function readFullBuffer(reader: ReadableStreamBYOBReader, dest: ArrayBuffer): Promise<ArrayBuffer> {
    const total = dest.byteLength;
    let offset = 0;
    let result;
    while (offset < dest.byteLength) {
        result = await reader.read(new DataView(dest, offset));
        offset += result.value?.byteLength ?? 0;
        if (result.done) {
            if (offset != total) {
                throw new Error("Failed to read full size of dest");
            }
        } 
    }
    return result?.value?.buffer ?? dest; 
}
