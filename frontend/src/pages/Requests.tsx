import { useState, useMemo } from 'react'
import { Search, Download, ChevronLeft, ChevronRight, Database, AlertCircle } from 'lucide-react'
import { useRequestLogs, type RequestItem } from '@/hooks/useRequestLogs'
import { RequestDetail } from '@/components/logs/RequestDetail'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'

function StatusBadge({ status, cacheHit }: { status: string; cacheHit: boolean }) {
  if (cacheHit) {
    return <Badge className="bg-purple-100 text-purple-800 hover:bg-purple-100">Cached</Badge>
  }
  if (status === 'success') {
    return <Badge className="bg-green-100 text-green-800 hover:bg-green-100">Success</Badge>
  }
  if (status === 'error') {
    return <Badge className="bg-red-100 text-red-800 hover:bg-red-100">Error</Badge>
  }
  return <Badge variant="secondary">{status}</Badge>
}

function formatDate(iso: string | null): string {
  if (!iso) return '—'
  return new Date(iso).toLocaleString()
}

function formatCost(cost: string): string {
  const n = parseFloat(cost)
  if (n === 0) return '$0.0000'
  return `$${n.toFixed(6)}`
}

function downloadCSV(requests: RequestItem[]) {
  const headers = ['Timestamp', 'Trace ID', 'Model', 'Status', 'Tokens', 'Cost', 'Latency (ms)', 'Cache Hit']
  const rows = requests.map((r) => [
    formatDate(r.gateway_received_at),
    r.trace_id,
    r.model_routed || r.model_requested || '',
    r.status,
    r.total_tokens,
    r.total_cost,
    r.latency_total_ms ?? '',
    r.cache_hit ? 'Yes' : 'No',
  ])

  const csv = [headers, ...rows]
    .map((row) => row.map((cell) => `"${String(cell).replace(/"/g, '""')}"`).join(','))
    .join('\n')

  const blob = new Blob([csv], { type: 'text/csv' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `requests-${new Date().toISOString().slice(0, 10)}.csv`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

export default function RequestsPage() {
  const [limit] = useState(20)
  const [offset, setOffset] = useState(0)
  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState<string>('all')
  const [selectedRequest, setSelectedRequest] = useState<RequestItem | null>(null)
  const [detailOpen, setDetailOpen] = useState(false)

  const { data, isLoading, error } = useRequestLogs(limit, offset)

  const requests = data?.data || []

  const filteredRequests = useMemo(() => {
    return requests.filter((req) => {
      const matchesSearch =
        searchQuery === '' ||
        req.trace_id.toLowerCase().includes(searchQuery.toLowerCase()) ||
        (req.model_routed || '').toLowerCase().includes(searchQuery.toLowerCase()) ||
        (req.model_requested || '').toLowerCase().includes(searchQuery.toLowerCase())

      let matchesStatus = true
      if (statusFilter === 'success') {
        matchesStatus = req.status === 'success' && !req.cache_hit
      } else if (statusFilter === 'error') {
        matchesStatus = req.status === 'error'
      } else if (statusFilter === 'cached') {
        matchesStatus = req.cache_hit
      }

      return matchesSearch && matchesStatus
    })
  }, [requests, searchQuery, statusFilter])

  const handleRowClick = (req: RequestItem) => {
    setSelectedRequest(req)
    setDetailOpen(true)
  }

  const handlePrevPage = () => {
    setOffset((prev) => Math.max(0, prev - limit))
  }

  const handleNextPage = () => {
    if (requests.length === limit) {
      setOffset((prev) => prev + limit)
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Request Logs</h1>
          <p className="text-muted-foreground">
            {data?.total || 0} total requests
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => downloadCSV(filteredRequests)}
          disabled={filteredRequests.length === 0}
        >
          <Download className="mr-2 h-4 w-4" />
          Export CSV
        </Button>
      </div>

      {error && (
        <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4 text-destructive">
          Failed to load requests: {error.message}
        </div>
      )}

      {/* Filters */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search by trace ID or model..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-8"
          />
        </div>
        <Select value={statusFilter} onValueChange={setStatusFilter}>
          <SelectTrigger className="w-[160px]">
            <SelectValue placeholder="Filter by status" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All Status</SelectItem>
            <SelectItem value="success">Success</SelectItem>
            <SelectItem value="error">Error</SelectItem>
            <SelectItem value="cached">Cached</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Requests</CardTitle>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
            </div>
          ) : filteredRequests.length === 0 ? (
            <div className="py-12 text-center">
              <Database className="mx-auto h-12 w-12 text-muted-foreground" />
              <h3 className="mt-4 text-lg font-semibold">No requests found</h3>
              <p className="text-muted-foreground">
                {requests.length === 0
                  ? 'Make your first API call to see logs here.'
                  : 'No requests match your filters.'}
              </p>
            </div>
          ) : (
            <div className="rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Timestamp</TableHead>
                    <TableHead>Trace ID</TableHead>
                    <TableHead>Model</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Tokens</TableHead>
                    <TableHead>Cost</TableHead>
                    <TableHead>Latency</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filteredRequests.map((req) => (
                    <TableRow
                      key={req.id}
                      className="cursor-pointer hover:bg-muted/50"
                      onClick={() => handleRowClick(req)}
                    >
                      <TableCell className="text-sm text-muted-foreground">
                        {formatDate(req.gateway_received_at)}
                      </TableCell>
                      <TableCell className="font-mono text-xs">{req.trace_id}</TableCell>
                      <TableCell className="text-sm">
                        {req.model_routed || req.model_requested || '—'}
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={req.status} cacheHit={req.cache_hit} />
                      </TableCell>
                      <TableCell className="text-sm">{req.total_tokens}</TableCell>
                      <TableCell className="text-sm">{formatCost(req.total_cost)}</TableCell>
                      <TableCell className="text-sm">
                        {req.latency_total_ms !== null ? `${req.latency_total_ms}ms` : '—'}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}

          {/* Pagination */}
          <div className="mt-4 flex items-center justify-between">
            <p className="text-sm text-muted-foreground">
              Showing {offset + 1}–{offset + requests.length}
            </p>
            <div className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={handlePrevPage}
                disabled={offset === 0}
              >
                <ChevronLeft className="mr-1 h-4 w-4" />
                Previous
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={handleNextPage}
                disabled={requests.length < limit}
              >
                Next
                <ChevronRight className="ml-1 h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <RequestDetail
        request={selectedRequest}
        open={detailOpen}
        onOpenChange={setDetailOpen}
      />
    </div>
  )
}
