import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Skeleton } from '@/components/ui/skeleton'
import { Badge } from '@/components/ui/badge'

export interface RecentRequest {
  id: string
  timestamp: string
  model: string
  provider: string
  status: 'success' | 'error' | 'cached'
  tokens: number
  cost_usd: number
  latency_ms: number
}

interface RecentRequestsProps {
  requests: RecentRequest[]
  isLoading?: boolean
}

function formatLatency(ms: number) {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(2)}s`
}

function formatCost(cost: number) {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 4,
    maximumFractionDigits: 4,
  }).format(cost)
}

function formatTokens(tokens: number) {
  return new Intl.NumberFormat('en-US', { notation: 'compact' }).format(tokens)
}

function getStatusBadge(status: string) {
  switch (status) {
    case 'success':
      return <Badge variant="default">Success</Badge>
    case 'cached':
      return <Badge variant="secondary">Cached</Badge>
    case 'error':
      return <Badge variant="destructive">Error</Badge>
    default:
      return <Badge variant="outline">{status}</Badge>
  }
}

export function RecentRequests({ requests, isLoading }: RecentRequestsProps) {
  if (isLoading) {
    return (
      <div className="space-y-2">
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-8 w-full" />
      </div>
    )
  }

  return (
    <div className="rounded-md border">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Time</TableHead>
            <TableHead>Model</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Tokens</TableHead>
            <TableHead>Cost</TableHead>
            <TableHead>Latency</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {requests.length === 0 ? (
            <TableRow>
              <TableCell
                colSpan={6}
                className="text-center text-muted-foreground"
              >
                No recent requests.
              </TableCell>
            </TableRow>
          ) : (
            requests.map((req) => (
              <TableRow key={req.id}>
                <TableCell className="text-xs text-muted-foreground">
                  {new Date(req.timestamp).toLocaleTimeString()}
                </TableCell>
                <TableCell className="font-medium">{req.model}</TableCell>
                <TableCell>{getStatusBadge(req.status)}</TableCell>
                <TableCell>{formatTokens(req.tokens)}</TableCell>
                <TableCell>{formatCost(req.cost_usd)}</TableCell>
                <TableCell>{formatLatency(req.latency_ms)}</TableCell>
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  )
}
