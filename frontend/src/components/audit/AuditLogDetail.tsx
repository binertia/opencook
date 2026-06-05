import { Shield, Clock, MapPin, Globe, FileJson } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import type { AuditEntry } from '@/hooks/useAuditLog'

interface AuditLogDetailProps {
  entry: AuditEntry | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

function formatJson(data: unknown): string {
  return JSON.stringify(data, null, 2)
}

export function AuditLogDetail({ entry, open, onOpenChange }: AuditLogDetailProps) {
  if (!entry) return null

  const handleCopyJson = () => {
    navigator.clipboard.writeText(formatJson(entry))
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Shield className="h-5 w-5" />
            Audit Entry Details
          </DialogTitle>
          <DialogDescription>
            Entry ID: <span className="font-mono text-xs">{entry.id}</span>
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* Action and timestamp */}
          <div className="flex items-center justify-between">
            <Badge variant="default" className="text-sm">
              {entry.action}
            </Badge>
            <div className="flex items-center gap-1.5 text-sm text-muted-foreground">
              <Clock className="h-4 w-4" />
              {new Date(entry.created_at).toLocaleString()}
            </div>
          </div>

          <Separator />

          {/* Actor and resource */}
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <p className="text-muted-foreground">Actor</p>
              <p className="font-medium">
                {entry.user_id || (
                  <span className="text-muted-foreground italic">System</span>
                )}
              </p>
            </div>
            <div>
              <p className="text-muted-foreground">Resource</p>
              <p className="font-medium">
                {entry.entity_type}
                {entry.entity_id && (
                  <span className="ml-1 font-mono text-xs text-muted-foreground">
                    ({entry.entity_id})
                  </span>
                )}
              </p>
            </div>
          </div>

          {/* IP and user agent */}
          {(entry.ip_address || entry.user_agent) && (
            <>
              <Separator />
              <div className="grid grid-cols-2 gap-4 text-sm">
                {entry.ip_address && (
                  <div className="flex items-center gap-1.5">
                    <MapPin className="h-3.5 w-3.5 text-muted-foreground" />
                    <span className="font-mono text-xs">{entry.ip_address}</span>
                  </div>
                )}
                {entry.user_agent && (
                  <div className="flex items-center gap-1.5">
                    <Globe className="h-3.5 w-3.5 text-muted-foreground" />
                    <span className="truncate text-xs text-muted-foreground">
                      {entry.user_agent}
                    </span>
                  </div>
                )}
              </div>
            </>
          )}

          {/* Summary */}
          {entry.summary && (
            <>
              <Separator />
              <div>
                <p className="text-sm text-muted-foreground">Summary</p>
                <p className="text-sm">{entry.summary}</p>
              </div>
            </>
          )}

          {/* Before/After diff */}
          {(entry.old_values || entry.new_values) && (
            <>
              <Separator />
              <div className="grid grid-cols-2 gap-4">
                {entry.old_values && (
                  <div>
                    <p className="text-sm text-muted-foreground mb-1">Before</p>
                    <pre className="rounded-md bg-muted p-2 text-xs overflow-auto max-h-48">
                      {formatJson(entry.old_values)}
                    </pre>
                  </div>
                )}
                {entry.new_values && (
                  <div>
                    <p className="text-sm text-muted-foreground mb-1">After</p>
                    <pre className="rounded-md bg-muted p-2 text-xs overflow-auto max-h-48">
                      {formatJson(entry.new_values)}
                    </pre>
                  </div>
                )}
              </div>
            </>
          )}

          {/* Full JSON */}
          <Separator />
          <div>
            <div className="flex items-center justify-between mb-1">
              <p className="text-sm text-muted-foreground">Full Entry</p>
              <Button variant="ghost" size="sm" onClick={handleCopyJson}>
                <FileJson className="mr-1 h-3.5 w-3.5" />
                Copy JSON
              </Button>
            </div>
            <pre className="rounded-md bg-muted p-2 text-xs overflow-auto max-h-48">
              {formatJson(entry)}
            </pre>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
