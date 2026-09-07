import { getThumbnailTintColor } from '@stump/client'
import { formatBytes } from '@stump/client'
import { cn, IconButton, ProgressBar, Text, ToolTip } from '@stump/components'
import { FragmentType, graphql, useFragment, UserPermission } from '@stump/graphql'
import { RefreshCw } from 'lucide-react'
import pluralize from 'pluralize'
import { memo, useCallback, useMemo } from 'react'

import { Link, useAppContext } from '@/context'
import { usePreferences } from '@/hooks/usePreferences'
import { useTheme } from '@/hooks/useTheme'
import { usePaths } from '@/paths'
import { usePrefetchBooksAfterCursor } from '@/scenes/book/BooksAfterCursor'
import { isEbookExtension, isEbookReadProgress, readProgressPercent } from '@/utils/readingProgress'

import { ThumbnailImage } from '../thumbnail/ThumbnailImage'
import { usePrefetchBook } from './useBookOverview'
import useKoboSync from './useKoboSync'

export const BookCardFragment = graphql(`
	fragment BookCard on Media {
		id
		resolvedName
		extension
		isSelectedForKoboSync
		pages
		size
		status
		thumbnail {
			url
			metadata {
				averageColor
				colors {
					color
					percentage
				}
				thumbhash
			}
			height
			width
		}
		readProgress {
			percentageCompleted
			page
			updatedAt
			locator {
				href
			}
		}
		readHistory {
			__typename
			completedAt
		}
		createdAt
		libraryConfig {
			skipBookOverview
		}
	}
`)

type Props = {
	fragment: FragmentType<typeof BookCardFragment>
	readingLink?: boolean
	onSelect?: () => void
	fullWidth?: boolean
}

const BookCard = memo(function BookCard({
	fragment,
	readingLink,
	onSelect,
	fullWidth = true,
}: Props) {
	const data = useFragment(BookCardFragment, fragment)
	const paths = usePaths()
	const { checkPermission } = useAppContext()
	const { mutate: setKoboSync, isPending: isUpdatingKoboSync } = useKoboSync()

	const {
		preferences: { thumbnailRatio },
	} = usePreferences()
	const { isDarkVariant, getColor: getThemeColor } = useTheme()

	const prefetchBook = usePrefetchBook()
	const prefetchBooksAfterCursor = usePrefetchBooksAfterCursor()

	const prefetch = useCallback(
		() => Promise.all([prefetchBook(data.id), prefetchBooksAfterCursor(data.id)]),
		[prefetchBook, prefetchBooksAfterCursor, data.id],
	)

	const progress = useMemo(() => {
		if (!data.readProgress && !data.readHistory) {
			return null
		} else if (data.readProgress) {
			const percent = readProgressPercent(data.readProgress, data.pages, data.extension)
			if (percent != null) {
				return percent
			}
		} else if (data.readHistory?.length) {
			return 100
		}

		return null
	}, [data])

	const placeholderData = useMemo(() => {
		const meta = data.thumbnail.metadata
		if (!meta) return undefined
		return {
			averageColor: meta.averageColor,
			colors: meta.colors,
			thumbhash: meta.thumbhash,
		}
	}, [data.thumbnail.metadata])

	const href = useMemo(() => {
		if (onSelect) {
			return undefined
		}

		const shouldSkipOverview = data.libraryConfig?.skipBookOverview === true

		return readingLink || shouldSkipOverview
			? paths.bookReader(data.id, {
					isEpub: isEbookExtension(data.extension),
					page: isEbookExtension(data.extension)
						? undefined
						: (data.readProgress?.page ?? undefined),
				})
			: paths.bookOverview(data.id)
	}, [readingLink, data.id, data.extension, onSelect, data.readProgress, data.libraryConfig, paths])

	const isMissing = data.status === 'MISSING'
	const canToggleKoboSync =
		data.extension.toLowerCase() === 'epub' && checkPermission(UserPermission.AccessKoboSync)
	const koboSyncLabel = data.isSelectedForKoboSync
		? `Remove ${data.resolvedName} from Kobo sync`
		: `Sync ${data.resolvedName} to Kobo`
	const isEbookProgress = isEbookReadProgress(data.readProgress, data.extension)
	const pagesLeft = data.pages - (data.readProgress?.page || 0)
	const progressPercent = progress ?? 0

	const renderSubtitle = () => {
		if (isMissing) {
			return (
				<Text size="xs" className="text-warning uppercase">
					File Missing
				</Text>
			)
		}

		if (progressPercent > 0 && progressPercent < 100) {
			return (
				<div className="gap-1 flex items-center justify-between">
					<Text size="xs" variant="muted">
						{progressPercent}%
					</Text>
					{!isEbookProgress && (
						<Text size="xs" variant="muted">
							{pagesLeft} {pluralize('page', pagesLeft)} left
						</Text>
					)}
				</div>
			)
		} else if (progressPercent === 100) {
			return (
				<Text size="xs" variant="muted">
					Completed
				</Text>
			)
		}

		return (
			<Text size="xs" variant="muted">
				{formatBytes(data.size.valueOf())}
			</Text>
		)
	}

	const handleClick = onSelect ? () => onSelect() : undefined

	const Comp = href ? Link : 'div'
	const props = href ? { to: href } : {}

	const thumbnailAverageColor = placeholderData?.averageColor
	const backgroundColor = useMemo(() => {
		if (thumbnailAverageColor) {
			return getThumbnailTintColor(thumbnailAverageColor, { dark: isDarkVariant })
		}
		return (
			getThemeColor('thumbnail.stack.series') ??
			(isDarkVariant ? 'oklch(0.35 0.01 52.14)' : 'oklch(0.9 0.01 52.14)')
		)
	}, [thumbnailAverageColor, isDarkVariant, getThemeColor])

	return (
		<div
			className={cn('relative', fullWidth ? 'w-full' : 'w-40 sm:w-[10.666rem] md:w-48 shrink-0')}
		>
			{/* @ts-expect-error: It's okay */}
			<Comp
				{...props}
				onClick={handleClick}
				onMouseEnter={prefetch}
				className={cn(
					'group gap-1 relative flex w-full flex-col',
					'p-1 rounded-lg border border-transparent transition-colors duration-100',
					'focus-visible:outline-none',
				)}
			>
				<div
					className={cn(
						'-inset-0.5 absolute -z-10 rounded-thumbnail',
						'scale-95 opacity-0 duration-100',
						'group-hover:scale-100 group-hover:opacity-100',
						'group-focus-visible:scale-100 group-focus-visible:opacity-100',
					)}
					style={{ backgroundColor: backgroundColor }}
				/>

				<div className="relative w-full" style={{ aspectRatio: thumbnailRatio }}>
					<ThumbnailImage
						src={data.thumbnail.url}
						alt={data.resolvedName}
						size={{ width: '100%', height: '100%' }}
						placeholderData={placeholderData}
						lazy
						borderAndShadowStyle={{
							shadowColor: 'rgba(0, 0, 0, 0.15)',
							shadowRadius: 2,
						}}
					/>
				</div>

				{progressPercent > 0 && (
					<ProgressBar
						value={progressPercent}
						max={100}
						variant="primary-dark"
						size="sm"
						className="-mt-0.5"
					/>
				)}

				<div className="gap-0.5 px-0.5 flex h-[52px] flex-col">
					<Text
						size="sm"
						className="min-w-0 font-medium leading-tight line-clamp-2 whitespace-normal"
					>
						{data.resolvedName}
					</Text>
					{renderSubtitle()}
				</div>
			</Comp>

			{canToggleKoboSync && (
				<ToolTip content={koboSyncLabel} side="top">
					<IconButton
						variant={data.isSelectedForKoboSync ? 'default' : 'secondary'}
						rounded="full"
						size="sm"
						className="right-2 top-2 shadow-md absolute z-20"
						aria-label={koboSyncLabel}
						aria-pressed={data.isSelectedForKoboSync}
						aria-busy={isUpdatingKoboSync}
						title={koboSyncLabel}
						disabled={isUpdatingKoboSync}
						onClick={(event) => {
							event.preventDefault()
							event.stopPropagation()
							setKoboSync({
								mediaIds: [data.id],
								isSelected: !data.isSelectedForKoboSync,
							})
						}}
					>
						<span className="size-4 relative" aria-hidden="true">
							<RefreshCw
								className={cn('inset-0 size-4 absolute', {
									'motion-safe:animate-spin': isUpdatingKoboSync,
								})}
							/>
							<span className="inset-0 font-bold absolute flex items-center justify-center text-[9px] leading-none">
								k
							</span>
						</span>
					</IconButton>
				</ToolTip>
			)}
		</div>
	)
})

export default BookCard
