import { renderHook } from '@testing-library/react'

import { BookReadingActionsRef, useBookReadingActions } from '../useBookReadingActions'

const mocks = vi.hoisted(() => ({
	book: {
		id: 'book',
		extension: 'epub',
		readProgress: { page: 12, locator: { href: 'chapter-4.xhtml' } },
		readHistory: [],
	},
	bookReader: vi.fn((_id: string, options: unknown) => JSON.stringify(options)),
}))

vi.mock('@stump/client', () => ({
	EBOOK_EXTENSION: /epub/i,
	PDF_EXTENSION: /pdf/i,
	useGraphQLMutation: () => ({ mutate: vi.fn() }),
}))
vi.mock('@stump/graphql', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/graphql')>()),
	useFragment: () => mocks.book,
}))
vi.mock('@tanstack/react-query', () => ({
	useQueryClient: () => ({ invalidateQueries: vi.fn() }),
}))
vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ translate: (key: string) => key }),
}))
vi.mock('@/paths', () => ({ usePaths: () => ({ bookReader: mocks.bookReader }) }))

test('restarts EPUBs explicitly while resume keeps their saved position', () => {
	const { result } = renderHook(() => useBookReadingActions({} as BookReadingActionsRef))

	expect(JSON.parse(result.current.links.resume)).toEqual({ isEpub: true })
	expect(JSON.parse(result.current.links.fromBeginning)).toEqual({
		isEpub: true,
		startFromBeginning: true,
	})
	expect(JSON.parse(result.current.links.incognito)).toEqual({
		isEpub: true,
		isIncognito: true,
		startFromBeginning: true,
	})
})
