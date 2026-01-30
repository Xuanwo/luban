import { waitForDataAttribute, waitForLocatorCount } from '../lib/utils.mjs';

export async function runStarFavorites({ page }) {
  const starButton = page.getByTestId('task-star-button');
  await starButton.waitFor({ state: 'visible' });

  const favoriteItems = page.locator('[data-testid^="favorite-task-"]');
  await waitForLocatorCount(favoriteItems, 0, 5_000);

  await starButton.click();
  await waitForDataAttribute(starButton, 'aria-pressed', 'true', 10_000);
  await waitForLocatorCount(favoriteItems, 1, 20_000);

  await starButton.click();
  await waitForDataAttribute(starButton, 'aria-pressed', 'false', 10_000);
  await waitForLocatorCount(favoriteItems, 0, 20_000);
}

