import { type LucideIcon } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'

interface KpiCardProps {
  title: string
  value: string
  change?: {
    value: string
    direction: 'up' | 'down' | 'neutral'
  }
  icon: LucideIcon
  iconColor?: string
  borderColor?: string
  isLoading?: boolean
}

export function KpiCard({
  title,
  value,
  change,
  icon: Icon,
  iconColor = 'text-primary',
  borderColor = 'border-primary',
  isLoading,
}: KpiCardProps) {
  return (
    <Card className={cn('border-l-4', borderColor)}>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="text-sm font-medium text-muted-foreground">
          {title}
        </CardTitle>
        <Icon className={cn('h-5 w-5', iconColor)} />
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <Skeleton className="h-8 w-24" />
        ) : (
          <div className="text-2xl font-bold">{value}</div>
        )}
        {change && !isLoading && (
          <p
            className={cn(
              'mt-1 text-xs font-medium',
              change.direction === 'up' && 'text-green-600 dark:text-green-400',
              change.direction === 'down' && 'text-red-600 dark:text-red-400',
              change.direction === 'neutral' && 'text-muted-foreground'
            )}
          >
            {change.direction === 'up' && '↑ '}
            {change.direction === 'down' && '↓ '}
            {change.value}
          </p>
        )}
      </CardContent>
    </Card>
  )
}
