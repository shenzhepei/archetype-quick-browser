export class TaskPool {
  private active = 0
  private readonly queue: Array<() => void> = []

  constructor(private readonly concurrency: number) {
    if (!Number.isInteger(concurrency) || concurrency < 1) throw new RangeError('Pool concurrency must be a positive integer.')
  }

  async run<T>(task: () => Promise<T>): Promise<T> {
    if (this.active >= this.concurrency) await new Promise<void>((resolve) => this.queue.push(resolve))
    this.active += 1
    try {
      return await task()
    } finally {
      this.active -= 1
      this.queue.shift()?.()
    }
  }
}
