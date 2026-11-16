import { InterfaceLayout } from '@stump/graphql'

import { createLayoutStore } from '../layout'

beforeEach(() => localStorage.clear())

test('restores the items-per-page preference after recreating the layout store', () => {
	const options = { key: 'upgrade', storage: localStorage }
	const original = createLayoutStore(options)
	original.getState().setPageSize(40)
	original.getState().setLayout(InterfaceLayout.Table)

	const reloaded = createLayoutStore(options)
	expect(reloaded.getState().pageSize).toBe(40)
	expect(reloaded.getState().layout).toBe(InterfaceLayout.Table)
})

test('keeps older layout preferences when no page-size preference exists', () => {
	localStorage.setItem(
		'stump-upgrade-layout-store',
		JSON.stringify({ state: { layout: InterfaceLayout.Table }, version: 1 }),
	)

	const store = createLayoutStore({ key: 'upgrade', storage: localStorage })

	expect(store.getState().layout).toBe(InterfaceLayout.Table)
	expect(store.getState().pageSize).toBeUndefined()
})
