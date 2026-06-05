import { AlertTriangle, Loader2 } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'

interface RevokeKeyDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  keyName: string
  lastUsed: string | null
  onConfirm: () => void
  isPending: boolean
}

function formatRelativeTime(date: string | null) {
  if (!date) return 'Never'
  const d = new Date(date)
  const now = new Date()
  const diffMs = now.getTime() - d.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMins / 60)
  const diffDays = Math.floor(diffHours / 24)

  if (diffMins < 1) return 'Just now'
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 30) return `${diffDays}d ago`
  return d.toLocaleDateString()
}

export function RevokeKeyDialog({
  open,
  onOpenChange,
  keyName,
  lastUsed,
  onConfirm,
  isPending,
}: RevokeKeyDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-destructive">
            <AlertTriangle className="h-5 w-5" />
            Revoke API Key
          </DialogTitle>
          <DialogDescription asChild>
            <div className="space-y-3 text-sm text-muted-foreground">
              <p>
                Are you sure you want to revoke <strong>{keyName}</strong>? This action is{' '}
                <strong>immediate and irreversible</strong>.
              </p>
              <ul className="list-disc pl-5 space-y-1">
                <li>The key will stop working immediately.</li>
                <li>Any active requests using this key may fail.</li>
                <li>This cannot be undone — you will need to create a new key.</li>
              </ul>
              <p>
                Last used:{' '}
                <span className="font-medium text-foreground">
                  {formatRelativeTime(lastUsed)}
                </span>
              </p>
            </div>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={onConfirm} disabled={isPending}>
            {isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Revoke Key
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
