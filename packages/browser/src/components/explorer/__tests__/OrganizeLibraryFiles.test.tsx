import '@/__mocks__/pointerCapture'

import { UserPermission } from '@stump/graphql'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { act } from 'react'

import OrganizeLibraryFiles from '../OrganizeLibraryFiles'

const mocks = vi.hoisted(() => ({
	checkPermission: vi.fn(),
	explorerContext: {
		currentPath: '/library',
		rootPath: '/library',
	},
	isPending: false,
	libraryContext: { library: { id: 'library-1' } } as { library: { id: string } } | null,
	mutationOptions: undefined as
		| { onError?: (error: unknown) => void; onSuccess?: () => void }
		| undefined,
	organize: vi.fn(),
	toastError: vi.fn(),
	toastSuccess: vi.fn(),
}))

vi.mock('@stump/client', () => ({
	useGraphQLMutation: (_mutation: unknown, options: typeof mocks.mutationOptions) => {
		mocks.mutationOptions = options
		return { isPending: mocks.isPending, mutate: mocks.organize }
	},
}))

vi.mock('@/context', () => ({
	useAppContext: () => ({ checkPermission: mocks.checkPermission }),
}))

vi.mock('@/scenes/library/context', () => ({
	useLibraryContextSafe: () => mocks.libraryContext,
}))

vi.mock('../context', () => ({
	useFileExplorerContext: () => mocks.explorerContext,
}))

vi.mock('@stump/i18n', () => ({
	useLocaleContext: () => ({
		t: (key: string) =>
			({
				'fileExplorer.organizeLibrary.confirmation.confirm': 'Organize files',
				'fileExplorer.organizeLibrary.confirmation.description': 'Move matched EPUB files.',
				'fileExplorer.organizeLibrary.confirmation.title': 'Organize library files?',
				'fileExplorer.organizeLibrary.error': 'Failed to organize library files',
				'fileExplorer.organizeLibrary.label': 'Organize library files',
				'fileExplorer.organizeLibrary.pending': 'Library organization is being queued',
				'fileExplorer.organizeLibrary.queued': 'Library organization queued',
				'fileExplorer.organizeLibrary.rootOnly': 'Return to the library root to organize files',
			})[key] ?? key,
	}),
}))

vi.mock('sonner', () => ({
	toast: { error: mocks.toastError, success: mocks.toastSuccess },
}))

describe('OrganizeLibraryFiles', () => {
	beforeEach(() => {
		vi.clearAllMocks()
		mocks.checkPermission.mockReturnValue(true)
		mocks.explorerContext.currentPath = '/library'
		mocks.explorerContext.rootPath = '/library'
		mocks.isPending = false
		mocks.libraryContext = { library: { id: 'library-1' } }
		mocks.mutationOptions = undefined
	})

	it('requires confirmation before queuing organization', () => {
		render(<OrganizeLibraryFiles />)
		expect(mocks.checkPermission).toHaveBeenCalledWith(UserPermission.ManageLibrary)

		fireEvent.click(screen.getByRole('button', { name: 'Organize library files' }))
		expect(mocks.organize).not.toHaveBeenCalled()

		fireEvent.click(screen.getByRole('button', { name: 'Organize files' }))
		expect(mocks.organize).toHaveBeenCalledWith({ id: 'library-1' })
	})

	it('stays visible but disabled below the library root', () => {
		mocks.explorerContext.currentPath = '/library/Author'
		render(<OrganizeLibraryFiles />)

		const button = screen.getByRole('button', { name: 'Organize library files' })
		expect(button).toHaveAttribute('aria-disabled', 'true')
		fireEvent.click(button)
		expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
	})

	it('hides without a library context or management permission', () => {
		mocks.libraryContext = null
		const { rerender } = render(<OrganizeLibraryFiles />)
		expect(screen.queryByRole('button', { name: 'Organize library files' })).not.toBeInTheDocument()

		mocks.libraryContext = { library: { id: 'library-1' } }
		mocks.checkPermission.mockReturnValue(false)
		rerender(<OrganizeLibraryFiles />)
		expect(screen.queryByRole('button', { name: 'Organize library files' })).not.toBeInTheDocument()
	})

	it('reports queued and failed requests and restores focus', async () => {
		render(<OrganizeLibraryFiles />)
		const trigger = screen.getByRole('button', { name: 'Organize library files' })
		fireEvent.click(trigger)
		screen.getByRole('button', { name: 'Organize files' }).focus()

		act(() => mocks.mutationOptions?.onSuccess?.())
		expect(mocks.toastSuccess).toHaveBeenCalledWith('Library organization queued')
		await waitFor(() => expect(trigger).toHaveFocus())

		mocks.mutationOptions?.onError?.(new Error('Disk unavailable'))
		expect(mocks.toastError).toHaveBeenCalledWith('Failed to organize library files', {
			description: 'Disk unavailable',
		})
	})
})
