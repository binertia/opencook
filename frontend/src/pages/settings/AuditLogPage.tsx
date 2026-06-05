import { useState } from 'react'
import { Shield, ChevronLeft, ChevronRight, FileSpreadsheet, FileJson } from 'lucide-react'
import { useAuth } from '@/hooks/useAuth'
import { useAuditLog, type AuditLogFilters, type AuditEntry } from '@/hooks/useAuditLog'
import { AuditLogFilters as FilterBar } from '@/components/audit/AuditLogFilters'
import { AuditLogDetail } from '@/components/audit/AuditLogDetail'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
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

function exportToCsv(filename: string, rows: AuditEntry[]) {
  if (rows.length === 0) return
  const headers = [
    'id',
    'action',
    'entity_type',
    'entity_id',
    'user_id',
    'summary',
    'ip_address',
    'created_at',
  ]
  const csv = [
    headers.join(','),
    ...rows.map((row) =>
      headers
        .map((h) => {
          const val = (row as Record<string, unknown>)[h]
          const str =
            typeof val === 'number' || typeof val === 'boolean'
              ? String(val)
              : val == null
                ? ''
                : `"${String(val).replace(/"/g, '""')}"`
          return str
        })
        .join(',')
    ),
  ].join('\n')
  const blob = new Blob([csv], { type: 'text/csv' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

function exportToJson(filename: string, data: AuditEntry[]) {
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

export default function AuditLogPage() {
  const { org } = useAuth()
  const [filters, setFilters] = useState<AuditLogFilters>({})
  const [limit, setLimit] = useState(50)
  const [offset, setOffset] = useState(0)
  const [selectedEntry, setSelectedEntry] = useState<AuditEntry | null>(null)

  const { data, isLoading } = useAuditLog(org?.id || '', filters, limit, offset)

  const total = data?.total ?? 0
  const hasNext = offset + limit < total
  const hasPrev = offset > 0

  const handleExportCsv = () => {
    if (!data) return
    exportToCsv('audit-log.csv', data.data)
  }

  const handleExportJson = () => {
    if (!data) return
    exportToJson('audit-log.json', data.data)
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-3">
          <Shield className="h-8 w-8 text-primary" />
          <div>
            <h1 className="text-3xl font-bold tracking-tight">Audit Log</h1>
            <p className="text-muted-foreground">
              Security-relevant actions across your organization.
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={handleExportCsv}>
            <FileSpreadsheet className="mr-2 h-4 w-4" />
            CSV
          </Button>
          <Button variant="outline" size="sm" onClick={handleExportJson}>
            <FileJson className="mr-2 h-4 w-4" />
            JSON
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Filters</CardTitle>
          <CardDescription>Narrow down audit entries by action, resource, or ID.</CardDescription>
        </CardHeader>
        <CardContent>
          <FilterBar filters={filters} onChange={setFilters} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <CardTitle>Entries</CardTitle>
            <p className="text-sm text-muted-foreground">
              {total.toLocaleString()} total
            </p>
          </div>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
            </div>
          ) : (
            <div className="rounded-md border overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Timestamp</TableHead>
                    <TableHead>Action</TableHead>
                    <TableHead>Resource</TableHead>
                    <TableHead>Actor</TableHead>
                    <TableHead className="max-w-xs">Summary</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data?.data.length === 0 ? (
                    <TableRow>
                      <TableCell
                        colSpan={5}
                        className="text-center text-muted-foreground py-8"
                      >
                        No audit entries found.
                      </TableCell>
                    </TableRow>
                  ) : (
                    data?.data.map((entry) => (
                      <TableRow
                        key={entry.id}
                        className="cursor-pointer hover:bg-muted/50"
                        onClick={() => setSelectedEntry(entry)}
                      >
                        <TableCell className="text-sm whitespace-nowrap">
                          {new Date(entry.created_at).toLocaleString()}
                        </TableCell>
                        <TableCell>
                          <Badge variant="outline" className="text-xs">
                            {entry.action}
                          </Badge>
                        </TableCell>
                        <TableCell className="text-sm">
                          {entry.entity_type}
                          {entry.entity_id && (
                            <span className="ml-1 text-xs text-muted-foreground font-mono">
                              {entry.entity_id.slice(0, 8)}…
                            </span>
                          )}
                        </TableCell>
                        <TableCell className="text-sm">
                          {entry.user_id ? (
                            <span className="font-mono text-xs">{entry.user_id.slice(0, 8)}…</span>
                          ) : (
                            <span className="text-muted-foreground italic">System</span>
                          )}
                        </TableCell>
                        <TableCell className="text-sm max-w-xs truncate">
                          {entry.summary}
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </div>
          )}

          {/* Pagination */}
          <div className="flex items-center justify-between mt-4">
            <p className="text-sm text-muted-foreground">
              Showing {offset + 1}–{Math.min(offset + limit, total)} of {total}
            </p>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => setOffset((o) => Math.max(0, o - limit))}
                disabled={!hasPrev}
              >
                <ChevronLeft className="mr-1 h-4 w-4" />
                Previous
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setOffset((o) => o + limit)}
                disabled={!hasNext}
              >
                Next
                <ChevronRight className="ml-1 h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <AuditLogDetail
        entry={selectedEntry}
        open={!!selectedEntry}
        onOpenChange={() => setSelectedEntry(null)}
      />
    </div>
  )
}
