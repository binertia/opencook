import { useEffect, useState } from 'react'
import { cn } from '@/lib/utils'

interface BudgetProgressBarProps {
  percentage: number
  current: number
  limit: number
  className?: string
}

function getSegmentColor(pct: number): string {
  if (pct <= 50) return 'bg-green-500'
  if (pct <= 75) return 'bg-yellow-500'
  if (pct <= 90) return 'bg-orange-500'
  return 'bg-red-500'
}

function getLabel(pct: number): string {
  if (pct <= 50) return 'On track'
  if (pct <= 75) return 'Approaching limit'
  if (pct <= 90) return 'Warning'
  return 'Over budget'
}

export function BudgetProgressBar({
  percentage,
  current,
  limit,
  className,
}: BudgetProgressBarProps) {
  const [animatedWidth, setAnimatedWidth] = useState(0)

  useEffect(() => {
    const timer = setTimeout(() => setAnimatedWidth(percentage), 100)
    return () => clearTimeout(timer)
  }, [percentage])

  const colorClass = getSegmentColor(percentage)
  const label = getLabel(percentage)

  return (
    <div className={cn('space-y-2', className)}>
      <div className="flex items-center justify-between text-sm">
        <span className="text-muted-foreground">
          {new Intl.NumberFormat('en-US', {
            style: 'currency',
            currency: 'USD',
          }).format(current)}{' '}
          of{' '}
          {new Intl.NumberFormat('en-US', {
            style: 'currency',
            currency: 'USD',
          }).format(limit)}
        </span>
        <div className="flex items-center gap-2">
          <span className="font-medium">{percentage.toFixed(1)}%</span>
          <span
            className={cn(
              'text-xs px-2 py-0.5 rounded-full font-medium',
              percentage <= 50 && 'bg-green-100 text-green-700',
              percentage > 50 && percentage <= 75 && 'bg-yellow-100 text-yellow-700',
              percentage > 75 && percentage <= 90 && 'bg-orange-100 text-orange-700',
              percentage > 90 && 'bg-red-100 text-red-700'
            )}
          >
            {label}
          </span>
        </div>
      </div>

      {/* Progress bar with segment markers */}
      <div className="relative h-4 w-full rounded-full bg-muted overflow-hidden">
        {/* Segment markers */}
        <div className="absolute inset-0 z-10 flex">
          <div className="h-full w-[50%] border-r border-background/50" />
          <div className="h-full w-[25%] border-r border-background/50" />
          <div className="h-full w-[15%] border-r border-background/50" />
          <div className="h-full flex-1" />
        </div>

        {/* Fill */}
        <div
          className={cn('h-full transition-all duration-700 ease-out relative z-0', colorClass)}
          style={{ width: `${animatedWidth}%` }}
        />
      </div>

      {/* Legend */}
      <div className="flex gap-3 text-xs text-muted-foreground">
        <div className="flex items-center gap-1">
          <div className="h-2 w-2 rounded-full bg-green-500" />
          <span>0-50%</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="h-2 w-2 rounded-full bg-yellow-500" />
          <span>50-75%</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="h-2 w-2 rounded-full bg-orange-500" />
          <span>75-90%</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="h-2 w-2 rounded-full bg-red-500" />
          <span>90-100%</span>
        </div>
      </div>
    </div>
  )
}
