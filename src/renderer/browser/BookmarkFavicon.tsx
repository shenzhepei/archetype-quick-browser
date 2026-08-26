import { Globe2 } from 'lucide-react'
import { useEffect, useState } from 'react'

export function BookmarkFavicon({ favicon, size = 14 }: { favicon?: string; size?: number }): React.JSX.Element {
  const [failed, setFailed] = useState(false)
  useEffect(() => setFailed(false), [favicon])
  return (
    <span className="bookmark-favicon" style={{ width: size, height: size }}>
      {favicon && !failed
        ? <img src={favicon} alt="" width={size} height={size} onError={() => setFailed(true)} />
        : <Globe2 size={size} />}
    </span>
  )
}
