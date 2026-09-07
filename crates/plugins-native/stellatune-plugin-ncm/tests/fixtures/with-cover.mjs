// Inject artwork into the synthetic tone without changing its encrypted audio.
export function withCover(source, image, padding = 64) {
  let offset = 10;
  offset += 4 + source.readUInt32LE(offset);
  offset += 4 + source.readUInt32LE(offset) + 5;
  const oldCapacity = source.readUInt32LE(offset);
  const lengths = Buffer.alloc(8);
  lengths.writeUInt32LE(image.length + padding, 0);
  lengths.writeUInt32LE(image.length, 4);
  return Buffer.concat([
    source.subarray(0, offset), lengths, image, Buffer.alloc(padding),
    source.subarray(offset + 8 + oldCapacity),
  ]);
}
