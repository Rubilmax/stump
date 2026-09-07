import { useGraphQLMutation } from '@stump/client'
import { cn, ConfirmationModal, IconButton, ToolTip, usePrevious } from '@stump/components'
import { extractErrorMessage, graphql, UserPermission } from '@stump/graphql'
import { useLocaleContext } from '@stump/i18n'
import { FolderSync } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'

import { useAppContext } from '@/context'
import { useLibraryContextSafe } from '@/scenes/library/context'

import { useFileExplorerContext } from './context'

const mutation = graphql(`
	mutation OrganizeLibraryFiles($id: ID!) {
		organizeLibraryFiles(id: $id)
	}
`)

export default function OrganizeLibraryFiles() {
	const [isOpen, setIsOpen] = useState(false)
	const triggerRef = useRef<HTMLButtonElement>(null)
	const wasOpen = usePrevious(isOpen)
	const libraryContext = useLibraryContextSafe()
	const { checkPermission } = useAppContext()
	const { currentPath, rootPath } = useFileExplorerContext()
	const { t } = useLocaleContext()

	const { mutate: organize, isPending } = useGraphQLMutation(mutation, {
		onSuccess: () => {
			setIsOpen(false)
			toast.success(t(getKey('queued')))
		},
		onError: (error) => {
			toast.error(t(getKey('error')), {
				description: extractErrorMessage(error),
			})
		},
	})

	useEffect(() => {
		if (wasOpen && !isOpen) triggerRef.current?.focus()
	}, [isOpen, wasOpen])

	const library = libraryContext?.library
	if (!library || !checkPermission(UserPermission.ManageLibrary)) return null

	const isAtRoot = currentPath === rootPath
	const isDisabled = !isAtRoot || isPending
	const tooltip = isPending
		? t(getKey('pending'))
		: isAtRoot
			? t(getKey('label'))
			: t(getKey('rootOnly'))

	return (
		<>
			<ToolTip content={tooltip} side="left" size="sm">
				<IconButton
					ref={triggerRef}
					variant="ghost"
					size="xs"
					className={cn('hover:bg-accent', isDisabled && 'cursor-not-allowed opacity-50')}
					aria-label={t(getKey('label'))}
					aria-disabled={isDisabled}
					aria-busy={isPending}
					onClick={() => {
						if (!isDisabled) setIsOpen(true)
					}}
				>
					<FolderSync aria-hidden="true" className="h-4 w-4" />
				</IconButton>
			</ToolTip>

			<ConfirmationModal
				isOpen={isOpen}
				title={t(getKey('confirmation.title'))}
				description={t(getKey('confirmation.description'))}
				confirmText={t(getKey('confirmation.confirm'))}
				confirmIsLoading={isPending}
				confirmDisabled={isPending}
				onClose={() => setIsOpen(false)}
				onConfirm={() => organize({ id: library.id })}
			/>
		</>
	)
}

const LOCALE_KEY = 'fileExplorer.organizeLibrary'
const getKey = (key: string) => `${LOCALE_KEY}.${key}`
