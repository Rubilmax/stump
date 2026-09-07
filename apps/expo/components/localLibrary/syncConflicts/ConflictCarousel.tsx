import { useLocaleContext } from '@stump/i18n'
import { useCallback, useRef, useState } from 'react'
import { AccessibilityInfo, Platform, useWindowDimensions, View } from 'react-native'
import Carousel, { ICarouselInstance } from 'react-native-reanimated-carousel'
import { useSafeAreaInsets } from 'react-native-safe-area-context'

import { useTranslate } from '~/lib/hooks'

import { Button, Text } from '../../ui'
import { SyncConflictPage, SyncConflictPageProps } from './SyncConflictPage'
import { ConflictRecord } from './types'

//  grabber 20ish + sheet header 56ish + top padding 20ish
const FIXED_OVERHEAD = 96
const NAVIGATION_HEIGHT = 48

type Props = {
	records: ConflictRecord[]
} & Pick<SyncConflictPageProps, 'onAcceptBoth' | 'onApplySyncedSessionData'>

export function ConflictCarousel({ records, ...pageProps }: Props) {
	const { t } = useLocaleContext()
	const { t: tMobile } = useTranslate()
	const { width, height } = useWindowDimensions()

	const insets = useSafeAreaInsets()
	const carouselRef = useRef<ICarouselInstance>(null)

	const [activeIndex, setActiveIndex] = useState(0)

	const hasNavigation = records.length > 1
	const carouselHeight =
		height - FIXED_OVERHEAD - insets.bottom - (hasNavigation ? NAVIGATION_HEIGHT : 0)
	const onSnapToItem = useCallback(
		(index: number) => {
			setActiveIndex(index)
			if (Platform.OS === 'ios') {
				AccessibilityInfo.announceForAccessibility(
					tMobile('common.pageXOfY', { current: index + 1, total: records.length }),
				)
			}
		},
		[records.length, tMobile],
	)

	return (
		<View className="flex-1">
			<Carousel
				ref={carouselRef}
				width={width}
				height={carouselHeight}
				data={records}
				loop={false}
				onSnapToItem={onSnapToItem}
				renderItem={({ item, index }) => (
					<SyncConflictPage
						record={item}
						isWithinLoadingRange={Math.abs(activeIndex - index) <= 1}
						// ^ lil leeway to load next before actually visible
						{...pageProps}
					/>
				)}
			/>

			{hasNavigation && (
				<View className="h-12 px-4 flex-row items-center justify-between">
					<Button
						variant="outline"
						disabled={activeIndex === 0}
						onPress={() => carouselRef.current?.prev()}
					>
						<Text>{t('pagination.buttons.previous')}</Text>
					</Button>

					<Text accessibilityLiveRegion="polite">
						{tMobile('common.pageXOfY', {
							current: activeIndex + 1,
							total: records.length,
						})}
					</Text>

					<Button
						variant="outline"
						disabled={activeIndex === records.length - 1}
						onPress={() => carouselRef.current?.next()}
					>
						<Text>{t('pagination.buttons.next')}</Text>
					</Button>
				</View>
			)}
		</View>
	)
}
