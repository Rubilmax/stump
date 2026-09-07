import { renderHook } from '@testing-library/react'

import useKoboSync from '../useKoboSync'

type Query = { queryKey: readonly unknown[] }
type Invalidation = { predicate: (query: Query) => boolean }
type MutationOptions = { onSuccess: () => unknown }

const mocks = vi.hoisted(() => ({
	cacheKeys: {
		bookOverview: 'bookOverview',
		libraryBooks: 'libraryBooks',
		seriesBooks: 'seriesBooks',
		smartListItems: 'smartListItems',
	},
	invalidateQueries: vi.fn<(options: Invalidation) => unknown>(),
	mutationOptions: undefined as MutationOptions | undefined,
}))

vi.mock('@stump/client', () => ({
	useGraphQLMutation: (_document: unknown, options: MutationOptions) => {
		mocks.mutationOptions = options
		return {}
	},
	useSDK: () => ({ sdk: { cacheKeys: mocks.cacheKeys } }),
}))
vi.mock('@tanstack/react-query', () => ({
	useQueryClient: () => ({ invalidateQueries: mocks.invalidateQueries }),
}))

describe('useKoboSync', () => {
	it('invalidates every BookCard query family after a successful mutation', () => {
		renderHook(() => useKoboSync())
		mocks.mutationOptions?.onSuccess()

		const invalidation = mocks.invalidateQueries.mock.calls[0]?.[0]
		expect(invalidation).toBeDefined()

		const affectedKeys = [
			mocks.cacheKeys.bookOverview,
			mocks.cacheKeys.libraryBooks,
			mocks.cacheKeys.seriesBooks,
			mocks.cacheKeys.smartListItems,
			'booksSearch',
			'booksAfterCursor',
			'bookOverlay',
		]
		for (const queryKey of affectedKeys) {
			expect(invalidation?.predicate({ queryKey: [queryKey, { page: 1 }] })).toBe(true)
		}
		expect(invalidation?.predicate({ queryKey: ['unrelated'] })).toBe(false)
	})
})
