export async function exportHdd(page) {
  // Transfer the full image as one base64 string. Serializing millions of
  // individual numbers through Playwright can exhaust the CI timeout (#122).
  // Decode every byte on the host so callers retain exact equality/save checks.
  const encoded = await page.evaluate(() => {
    const bytes = pc98.exportDisk(2);
    let binary = '';
    for (let at = 0; at < bytes.length; at += 32768) {
      binary += String.fromCharCode(...bytes.slice(at, at + 32768));
    }
    return btoa(binary);
  });
  return Buffer.from(encoded, 'base64');
}
