import { useState } from 'react'
import { Copy, CheckCircle } from 'lucide-react'
import { useCreateWebhook } from '@/hooks/useWebhooks'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Card, CardContent } from '@/components/ui/card'

const EVENT_OPTIONS = [
  { value: 'request.completed', label: 'Request Completed', category: 'Requests' },
  { value: 'request.failed', label: 'Request Failed', category: 'Requests' },
  { value: 'quota.warning', label: 'Quota Warning', category: 'Quotas' },
  { value: 'quota.exceeded', label: 'Quota Exceeded', category: 'Quotas' },
  { value: 'provider.error', label: 'Provider Error', category: 'Providers' },
  { value: 'provider.recovered', label: 'Provider Recovered', category: 'Providers' },
]

interface CreateWebhookModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function CreateWebhookModal({ open, onOpenChange }: CreateWebhookModalProps) {
  const createWebhook = useCreateWebhook()

  const [name, setName] = useState('')
  const [url, setUrl] = useState('')
  const [selectedEvents, setSelectedEvents] = useState<string[]>([])
  const [createdSecret, setCreatedSecret] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  const handleToggleEvent = (event: string) => {
    setSelectedEvents((prev) =>
      prev.includes(event) ? prev.filter((e) => e !== event) : [...prev, event]
    )
  }

  const handleCreate = () => {
    if (!name.trim() || !url.trim() || selectedEvents.length === 0) return
    createWebhook.mutate(
      { name: name.trim(), url: url.trim(), events: selectedEvents },
      {
        onSuccess: (data) => {
          setCreatedSecret(data.secret)
        },
      }
    )
  }

  const handleCopy = (text: string) => {
    navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const handleClose = () => {
    setName('')
    setUrl('')
    setSelectedEvents([])
    setCreatedSecret(null)
    setCopied(false)
    onOpenChange(false)
  }

  const isValid =
    name.trim().length > 0 &&
    url.trim().length > 0 &&
    selectedEvents.length > 0 &&
    (url.startsWith('https://') || url.startsWith('http://'))

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Create Webhook</DialogTitle>
          <DialogDescription>
            {createdSecret
              ? 'Copy your signing secret now. You will not be able to see it again.'
              : 'Configure a new webhook to receive event notifications.'}
          </DialogDescription>
        </DialogHeader>

        {createdSecret ? (
          <div className="space-y-4">
            <Card className="border-green-200 bg-green-50">
              <CardContent className="pt-6">
                <Label className="text-green-800">Signing Secret</Label>
                <div className="mt-2 flex items-center gap-2">
                  <code className="flex-1 rounded bg-white px-3 py-2 text-sm font-mono break-all border">
                    {createdSecret}
                  </code>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => handleCopy(createdSecret)}
                  >
                    {copied ? (
                      <CheckCircle className="h-4 w-4 text-green-600" />
                    ) : (
                      <Copy className="h-4 w-4" />
                    )}
                  </Button>
                </div>
                <p className="mt-2 text-xs text-green-700">
                  Use this secret to verify webhook signatures. Store it securely.
                </p>
              </CardContent>
            </Card>

            <DialogFooter>
              <Button onClick={handleClose}>Done</Button>
            </DialogFooter>
          </div>
        ) : (
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="webhook-name">Name</Label>
              <Input
                id="webhook-name"
                placeholder="e.g. Production Alerts"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="webhook-url">Endpoint URL</Label>
              <Input
                id="webhook-url"
                placeholder="https://example.com/webhook"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
              />
              {url && !url.startsWith('https://') && !url.startsWith('http://') && (
                <p className="text-xs text-destructive">URL must start with http:// or https://</p>
              )}
            </div>

            <div className="space-y-2">
              <Label>Events</Label>
              <div className="space-y-3">
                {['Requests', 'Quotas', 'Providers'].map((category) => (
                  <div key={category}>
                    <p className="text-xs font-medium text-muted-foreground mb-1">{category}</p>
                    <div className="grid grid-cols-2 gap-2">
                      {EVENT_OPTIONS.filter((e) => e.category === category).map((event) => (
                        <label
                          key={event.value}
                          htmlFor={event.value}
                          className="flex items-center gap-2 cursor-pointer"
                        >
                          <input
                            id={event.value}
                            type="checkbox"
                            checked={selectedEvents.includes(event.value)}
                            onChange={() => handleToggleEvent(event.value)}
                            className="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
                          />
                          <span className="text-sm">{event.label}</span>
                        </label>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
              {selectedEvents.length === 0 && (
                <p className="text-xs text-destructive">Select at least one event</p>
              )}
            </div>

            <DialogFooter>
              <Button variant="ghost" onClick={handleClose}>
                Cancel
              </Button>
              <Button
                onClick={handleCreate}
                disabled={createWebhook.isPending || !isValid}
              >
                {createWebhook.isPending ? 'Creating...' : 'Create Webhook'}
              </Button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
