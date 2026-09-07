import { FileStatus, makeFragmentData, UserPermission } from '@stump/graphql'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import BookCard, { BookCardFragment } from '../BookCard'

const mocks = vi.hoisted(() => {
	const state = {
		canAccessKobo: true,
		isUpdatingKoboSync: false,
	}
	return {
		checkPermission: vi.fn(() => state.canAccessKobo),
		setKoboSync: vi.fn(),
		state,
	}
})

vi.mock('@/context', () => ({
	Link: ({ to, ...props }: Omit<React.ComponentPropsWithoutRef<'a'>, 'href'> & { to: string }) => (
		<a href={to} {...props} />
	),
	useAppContext: () => ({
		checkPermission: mocks.checkPermission,
	}),
}))
vi.mock('@/hooks/usePreferences', () => ({
	usePreferences: () => ({ preferences: { thumbnailRatio: 2 / 3 } }),
}))
vi.mock('@/hooks/useTheme', () => ({
	useTheme: () => ({ getColor: () => undefined, isDarkVariant: false }),
}))
vi.mock('@/paths', () => ({
	usePaths: () => ({
		bookOverview: (id: string) => `/books/${id}`,
		bookReader: (id: string) => `/books/${id}/read`,
	}),
}))
vi.mock('@/scenes/book/BooksAfterCursor', () => ({
	usePrefetchBooksAfterCursor: () => vi.fn(),
}))
vi.mock('../../thumbnail/ThumbnailImage', () => ({
	ThumbnailImage: ({ alt }: { alt?: string }) => <div aria-label={alt} role="img" />,
}))
vi.mock('../useBookOverview', () => ({ usePrefetchBook: () => vi.fn() }))
vi.mock('../useKoboSync', () => ({
	default: () => ({
		isPending: mocks.state.isUpdatingKoboSync,
		mutate: mocks.setKoboSync,
	}),
}))

const makeBook = (extension = 'epub', isSelectedForKoboSync = true) =>
	makeFragmentData(
		{
			createdAt: '2026-09-07T00:00:00Z',
			extension,
			id: 'book-1',
			isSelectedForKoboSync,
			libraryConfig: { skipBookOverview: false },
			pages: 100,
			readHistory: [],
			readProgress: null,
			resolvedName: 'Dune',
			size: 1024,
			status: FileStatus.Ready,
			thumbnail: { height: 300, metadata: null, url: '/cover.jpg', width: 200 },
		},
		BookCardFragment,
	)

describe('BookCard Kobo sync toggle', () => {
	beforeEach(() => {
		vi.clearAllMocks()
		mocks.state.canAccessKobo = true
		mocks.state.isUpdatingKoboSync = false
	})

	it('toggles Kobo sync without selecting the card', async () => {
		const onSelect = vi.fn()
		render(<BookCard fragment={makeBook()} onSelect={onSelect} />)

		const toggle = screen.getByRole('button', { name: 'Remove Dune from Kobo sync' })
		expect(toggle).toHaveAttribute('aria-pressed', 'true')
		await userEvent.hover(toggle)
		expect(await screen.findByRole('tooltip')).toHaveTextContent('Remove Dune from Kobo sync')

		await userEvent.click(toggle)

		expect(mocks.setKoboSync).toHaveBeenCalledWith({
			isSelected: false,
			mediaIds: ['book-1'],
		})
		expect(onSelect).not.toHaveBeenCalled()
	})

	it('keeps the toggle outside the card link and does not navigate', async () => {
		window.history.replaceState(null, '', '/library')
		render(<BookCard fragment={makeBook()} />)

		const link = screen.getByRole('link')
		const toggle = screen.getByRole('button', { name: 'Remove Dune from Kobo sync' })
		expect(link).not.toContainElement(toggle)

		await userEvent.click(toggle)

		expect(window.location.pathname).toBe('/library')
	})

	it.each([
		['non-EPUB books', 'pdf', true],
		['users without permission', 'epub', false],
	])('hides the toggle for %s', (_, extension, canAccessKobo) => {
		mocks.state.canAccessKobo = canAccessKobo
		render(<BookCard fragment={makeBook(extension)} onSelect={vi.fn()} />)

		expect(screen.queryByRole('button', { name: /Kobo/ })).not.toBeInTheDocument()
		if (!canAccessKobo) {
			expect(mocks.checkPermission).toHaveBeenCalledWith(UserPermission.AccessKoboSync)
		}
	})

	it('disables the toggle while its mutation is pending', () => {
		mocks.state.isUpdatingKoboSync = true
		render(<BookCard fragment={makeBook('epub', false)} onSelect={vi.fn()} />)

		const toggle = screen.getByRole('button', { name: 'Sync Dune to Kobo' })
		expect(toggle).toBeDisabled()
		expect(toggle).toHaveAttribute('aria-busy', 'true')
	})
})
