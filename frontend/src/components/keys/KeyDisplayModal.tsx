import { useState } from 'react'
import { Copy, Check, Download, AlertTriangle } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'

interface KeyDisplayModalProps {
  apiKey: string | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function KeyDisplayModal({ apiKey, open, onOpenChange }: KeyDisplayModalProps) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    if (!apiKey) return
    await navigator.clipboard.writeText(apiKey)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const handleDownload = () => {
    if (!apiKey) return
    const blob = new Blob([apiKey], { type: 'text/plain' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'api-key.txt'
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-destructive">
            <AlertTriangle className="h-5 w-5" />
            Save Your API Key
          </DialogTitle>
          <DialogDescription>
            This key will <strong>never be shown again</strong>. Copy it now or download it to a
            secure location.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="rounded-md border border-destructive/30 bg-destructive/5 p-4">
            <code className="block break-all font-mono text-sm">{apiKey}</code>
          </div>

          <div className="flex gap-2">
            <Button
              variant={copied ? 'default' : 'outline'}
              className="flex-1"
              onClick={handleCopy}
            >
              {copied ? (
                <>
                  <Check className="mr-2 h-4 w-4" />
                  Copied!
                </>
              ) : (
                <>
                  <Copy className="mr-2 h-4 w-4" />
                  Copy to Clipboard
                </>
              )}
            </Button>
            <Button variant="outline" className="flex-1" onClick={handleDownload}>
              <Download className="mr-2 h-4 w-4" />
              Download
            </Button>
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            I have saved my key
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
