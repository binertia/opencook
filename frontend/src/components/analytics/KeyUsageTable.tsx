import { useState } from 'react'
import {
  ArrowUpDown,
  ArrowUp,
  ArrowDown,
  KeyRound,
} from 'lucide-react'
import type { KeyUsageItem } from '@/hooks/useKeyUsage'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'

type SortKey = 'key_name' | 'requests' | 'tokens' | 'cost_usd' | 'avg_latency_ms'
type SortOrder = 'asc' | 'desc'

interface KeyUsageTableProps {
  data: KeyUsageItem[]
  onRowClick: (keyId: string) => void
}

export function KeyUsageTable({ data, onRowClick }: KeyUsageTableProps) {
  const [sortBy, setSortBy] = useState<SortKey>('cost_usd')
  const [sortOrder, setSortOrder] = useState<SortOrder>('desc')

  const handleSort = (key: SortKey) => {
    if (sortBy === key) {
      setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc')
    } else {
      setSortBy(key)
      setSortOrder('desc')
    }
  }

  const sorted = [...data].sort((a, b) => {
    const aVal = a[sortBy]
    const bVal = b[sortBy]
    if (typeof aVal === 'string' && typeof bVal === 'string') {
      return sortOrder === 'asc'
        ? aVal.localeCompare(bVal)
        : bVal.localeCompare(aVal)
    }
    const aNum = typeof aVal === 'number' ? aVal : 0
    const bNum = typeof bVal === 'number' ? bVal : 0
    return sortOrder === 'asc' ? aNum - bNum : bNum - aNum
  })

  const SortIcon = ({ column }: { column: SortKey }) => {
    if (sortBy !== column) return <ArrowUpDown className="ml-1 h-3 w-3" />
    return sortOrder === 'asc' ? (
      <ArrowUp className="ml-1 h-3 w-3" />
    ) : (
      <ArrowDown className="ml-1 h-3 w-3" />
    )
  }

  return (
    <div className="rounded-md border overflow-x-auto">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>
              <Button
                variant="ghost"
                size="sm"
                className="-ml-2 h-8"
                onClick={() => handleSort('key_name')}
              >
                Key <SortIcon column="key_name" />
              </Button>
            </TableHead>
            <TableHead>Status</TableHead>
            <TableHead className="text-right">
              <Button
                variant="ghost"
                size="sm"
                className="-mr-2 h-8"
                onClick={() => handleSort('requests')}
              >
                Requests <SortIcon column="requests" />
              </Button>
            </TableHead>
            <TableHead className="text-right">
              <Button
                variant="ghost"
                size="sm"
                className="-mr-2 h-8"
                onClick={() => handleSort('tokens')}
              >
                Tokens <SortIcon column="tokens" />
              </Button>
            </TableHead>
            <TableHead className="text-right">
              <Button
                variant="ghost"
                size="sm"
                className="-mr-2 h-8"
                onClick={() => handleSort('cost_usd')}
              >
                Cost <SortIcon column="cost_usd" />
              </Button>
            </TableHead>
            <TableHead className="text-right">
              <Button
                variant="ghost"
                size="sm"
                className="-mr-2 h-8"
                onClick={() => handleSort('avg_latency_ms')}
              >
                Avg Latency <SortIcon column="avg_latency_ms" />
              </Button>
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {sorted.length === 0 ? (
            <TableRow>
              <TableCell
                colSpan={6}
                className="text-center text-muted-foreground py-8"
              >
                No API key usage data for the selected period.
              </TableCell>
            </TableRow>
          ) : (
            sorted.map((item) => (
              <TableRow
                key={item.api_key_id}
                className="cursor-pointer hover:bg-muted/50"
                onClick={() => onRowClick(item.api_key_id)}
              >
                <TableCell>
                  <div className="flex items-center gap-2">
                    <KeyRound className="h-4 w-4 text-muted-foreground" />
                    <div>
                      <p className="text-sm font-medium">{item.key_name}</p>
                      <p className="text-xs text-muted-foreground font-mono">
                        {item.key_prefix}…
                      </p>
                    </div>
                  </div>
                </TableCell>
                <TableCell>
                  <Badge
                    variant={item.key_status === 'active' ? 'default' : 'secondary'}
                  >
                    {item.key_status}
                  </Badge>
                </TableCell>
                <TableCell className="text-right text-sm">
                  {new Intl.NumberFormat('en-US').format(item.requests)}
                </TableCell>
                <TableCell className="text-right text-sm">
                  {new Intl.NumberFormat('en-US').format(item.tokens)}
                </TableCell>
                <TableCell className="text-right text-sm font-medium">
                  {new Intl.NumberFormat('en-US', {
                    style: 'currency',
                    currency: 'USD',
                  }).format(item.cost_usd)}
                </TableCell>
                <TableCell className="text-right text-sm">
                  {item.avg_latency_ms > 0
                    ? `${item.avg_latency_ms}ms`
                    : '—'}
                </TableCell>
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  )
}
