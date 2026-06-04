import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'
import type { ProviderHealth } from '@/hooks/useProviders'

interface HealthIndicatorProps {
  health?: ProviderHealth
  size?: 'sm' | 'md' | 'lg'
}

const STATUS_COLORS = {
  healthy: 'bg-green-500',
  degraded: 'bg-yellow-500',
  unhealthy: 'bg-red-500',
  unknown: 'bg-gray-400',
}

const STATUS_LABELS = {
  healthy: 'Healthy',
  degraded: 'Degraded',
  unhealthy: 'Unhealthy',
  unknown: 'Unknown',
}

const SIZE_CLASSES = {
  sm: 'h-2 w-2',
  md: 'h-2.5 w-2.5',
  lg: 'h-3 w-3',
}

export function HealthIndicator({ health, size = 'md' }: HealthIndicatorProps) {
  const status = health?.status || 'unknown'
  const color = STATUS_COLORS[status]
  const label = STATUS_LABELS[status]

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <div className="flex items-center gap-2 cursor-help">
            <div className={cn('rounded-full', color, SIZE_CLASSES[size])} />
            <span className="text-sm capitalize">{label}</span>
          </div>
        </TooltipTrigger>
        <TooltipContent className="space-y-1">
          <p className="font-semibold">{label}</p>
          {health && (
            <>
              <p className="text-xs text-muted-foreground">
                Latency: {health.latency_ms}ms
              </p>
              <p className="text-xs text-muted-foreground">
                Error rate: {(health.error_rate * 100).toFixed(2)}%
              </p>
              <p className="text-xs text-muted-foreground">
                Last checked: {new Date(health.last_checked).toLocaleString()}
              </p>
              {health.message && (
                <p className="text-xs text-muted-foreground">
                  {health.message}
                </p>
              )}
            </>
          )}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}
