import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Badge } from '@/components/ui/badge'

export interface AlertRecord {
  id: string
  timestamp: string
  threshold_percent: number
  current_spend_usd: number
  budget_limit_usd: number
  notification_sent: boolean
  channel: 'email' | 'webhook' | 'dashboard'
}

interface AlertHistoryProps {
  alerts: AlertRecord[]
}

export function AlertHistory({ alerts }: AlertHistoryProps) {
  return (
    <div className="rounded-md border">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Timestamp</TableHead>
            <TableHead>Threshold</TableHead>
            <TableHead className="text-right">Spend</TableHead>
            <TableHead className="text-right">Limit</TableHead>
            <TableHead>Notification</TableHead>
            <TableHead>Channel</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {alerts.length === 0 ? (
            <TableRow>
              <TableCell
                colSpan={6}
                className="text-center text-muted-foreground py-8"
              >
                No alerts triggered yet.
              </TableCell>
            </TableRow>
          ) : (
            alerts.map((alert) => (
              <TableRow key={alert.id}>
                <TableCell className="text-sm">
                  {new Date(alert.timestamp).toLocaleString()}
                </TableCell>
                <TableCell>
                  <Badge
                    variant={
                      alert.threshold_percent >= 100
                        ? 'destructive'
                        : alert.threshold_percent >= 90
                          ? 'secondary'
                          : 'default'
                    }
                  >
                    {alert.threshold_percent}%
                  </Badge>
                </TableCell>
                <TableCell className="text-right text-sm">
                  {new Intl.NumberFormat('en-US', {
                    style: 'currency',
                    currency: 'USD',
                  }).format(alert.current_spend_usd)}
                </TableCell>
                <TableCell className="text-right text-sm">
                  {new Intl.NumberFormat('en-US', {
                    style: 'currency',
                    currency: 'USD',
                  }).format(alert.budget_limit_usd)}
                </TableCell>
                <TableCell>
                  {alert.notification_sent ? (
                    <Badge variant="outline" className="text-green-600">
                      Sent
                    </Badge>
                  ) : (
                    <Badge variant="outline" className="text-muted-foreground">
                      Pending
                    </Badge>
                  )}
                </TableCell>
                <TableCell>
                  <span className="text-sm capitalize">{alert.channel}</span>
                </TableCell>
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  )
}
