import { sleep } from '../lib/utils.mjs'

async function inboxRowTaskTitle(row) {
	const title = row.getByTestId('inbox-notification-task-title').first()
	await title.waitFor({ state: 'visible' })
	return ((await title.textContent()) ?? '').trim()
}

async function findInboxRowIndexByTitle({ page, title }) {
	const rows = page.locator('[data-testid^="inbox-notification-row-"]')
	const count = await rows.count()
	for (let i = 0; i < count; i += 1) {
		const row = rows.nth(i)
		const text = await inboxRowTaskTitle(row)
		if (text === title) return i
	}
	return -1
}

async function waitForInboxTitlePresence({ page, title, shouldExist, timeoutMs = 20_000 }) {
	const start = Date.now()
	while (Date.now() - start < timeoutMs) {
		const idx = await findInboxRowIndexByTitle({ page, title })
		if (shouldExist ? idx !== -1 : idx === -1) return
		await sleep(200)
	}
	const idx = await findInboxRowIndexByTitle({ page, title })
	throw new Error(`timeout waiting for inbox title presence; title=${JSON.stringify(title)} shouldExist=${shouldExist} idx=${idx}`)
}

async function waitForInboxRows({ page, timeoutMs = 20_000 }) {
	const rows = page.locator('[data-testid^="inbox-notification-row-"]')
	const start = Date.now()
	while (Date.now() - start < timeoutMs) {
		if ((await rows.count()) > 0) return
		await sleep(200)
	}
	throw new Error('timeout waiting for inbox rows')
}

export async function runInboxFilter({ page }) {
	await page.getByTestId('nav-inbox-button').click()
	await page.getByTestId('inbox-view').waitFor({ state: 'visible' })
	await waitForInboxRows({ page })

	// Default filter hides done tasks.
	await waitForInboxTitlePresence({ page, title: 'Done: completed successfully', shouldExist: false })

	// Time filtering should affect older fixtures.
	await waitForInboxTitlePresence({ page, title: 'Mock: 10 days old', shouldExist: true })
	await waitForInboxTitlePresence({ page, title: 'Mock: 45 days old', shouldExist: true })

	await page.getByTestId('inbox-filter-button').click()
	await page.getByTestId('inbox-filter-popover').waitFor({ state: 'visible' })

	await page.getByTestId('inbox-filter-section-updated-toggle').click()
	await page.getByTestId('inbox-filter-updated-2w').click()
	await waitForInboxTitlePresence({ page, title: 'Mock: 10 days old', shouldExist: true })
	await waitForInboxTitlePresence({ page, title: 'Mock: 45 days old', shouldExist: false })

	await page.getByTestId('inbox-filter-updated-1w').click()
	await waitForInboxTitlePresence({ page, title: 'Mock: 10 days old', shouldExist: false })

	await page.getByTestId('inbox-filter-updated-any').click()
	await waitForInboxTitlePresence({ page, title: 'Mock: 10 days old', shouldExist: true })
	await waitForInboxTitlePresence({ page, title: 'Mock: 45 days old', shouldExist: true })

	// Status filter: include done tasks.
	await page.getByTestId('inbox-filter-section-status-toggle').click()
	await page.getByTestId('inbox-filter-status-done').click()
	await page.getByTestId('inbox-filter-close').click()
	await waitForInboxTitlePresence({ page, title: 'Done: completed successfully', shouldExist: true })

	// Project filter: narrow to project 2.
	await page.getByTestId('inbox-filter-button').click()
	await page.getByTestId('inbox-filter-popover').waitFor({ state: 'visible' })
	await page.getByTestId('inbox-filter-section-project-toggle').click()
	await page.getByTestId('inbox-filter-project-mock-project-2').click()
	await page.getByTestId('inbox-filter-close').click()

	await waitForInboxTitlePresence({ page, title: 'Local task', shouldExist: true })
	await waitForInboxTitlePresence({ page, title: 'Mock task 1', shouldExist: false })

	// Restore defaults for later scenarios.
	await page.getByTestId('inbox-filter-button').click()
	await page.getByTestId('inbox-filter-popover').waitFor({ state: 'visible' })
	await page.getByTestId('inbox-filter-clear').click()
	await page.getByTestId('inbox-filter-close').click()

	await waitForInboxTitlePresence({ page, title: 'Done: completed successfully', shouldExist: false })
	await waitForInboxTitlePresence({ page, title: 'Mock task 1', shouldExist: true })
}
