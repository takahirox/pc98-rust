// Use the same picker, chooser and settings controls as a player.
export async function uploadMedia(page, files, drive = 'auto') {
  await page.locator(`#drive-${drive}`).click();
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.locator('#choose-file').click(),
  ]);
  await chooser.setFiles(files);
}

export async function diskAction(page, drive, action) {
  await page.locator(`#drive-${drive}`).click();
  await page.locator(`#${action}`).click();
}

export async function openSettings(page) {
  if (!await page.locator('#settings-panel').isVisible()) await page.locator('#settings').click();
}

export async function nextBoot(page, source) {
  await openSettings(page);
  await page.locator('#boot').selectOption(source);
  await page.keyboard.press('Escape');
}

export async function launchDemo(page, demo, { touch = false } = {}) {
  const action = touch ? 'tap' : 'click';
  await page.locator('#demos')[action]();
  await page.locator(`#${demo}`)[action]();
}
