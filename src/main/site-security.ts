import type { Certificate, PermissionRequest, Session } from 'electron'
import { baseSiteInfo } from '../shared/browser'
import type { CertificateSummary, SiteInfo, SitePermissionRecord } from '../shared/browser'

export class SiteSecurityService {
  private readonly certificates = new Map<string, CertificateSummary>()
  private readonly permissions = new Map<string, Map<string, SitePermissionRecord>>()

  configure(browserSession: Session, onChange: () => void): void {
    browserSession.setCertificateVerifyProc((request, callback) => {
      this.certificates.set(request.hostname.toLowerCase(), this.certificateSummary(request.certificate, request.isIssuedByKnownRoot, request.verificationResult, request.errorCode))
      onChange()
      callback(-3)
    })
    browserSession.setPermissionRequestHandler((_webContents, permission, callback, details) => {
      const origin = this.originFrom(details)
      if (origin) {
        const records = this.permissions.get(origin) ?? new Map<string, SitePermissionRecord>()
        records.set(permission, { permission, state: 'blocked' })
        this.permissions.set(origin, records)
        onChange()
      }
      callback(false)
    })
  }

  infoFor(url: string): SiteInfo {
    const info = baseSiteInfo(url)
    if (info.origin) info.permissions = [...(this.permissions.get(info.origin)?.values() ?? [])]
    if (info.connection !== 'verifying') return info
    const hostname = new URL(url).hostname.toLowerCase()
    const certificate = this.certificates.get(hostname)
    if (!certificate) return info
    return {
      ...info,
      connection: certificate.errorCode === 0 || certificate.verificationResult === 'OK' || certificate.verificationResult === 'net::OK' ? 'secure' : 'insecure',
      certificate
    }
  }

  private originFrom(details: PermissionRequest): string | undefined {
    try {
      const origin = new URL(details.requestingUrl).origin
      return origin === 'null' ? undefined : origin
    } catch {
      return undefined
    }
  }

  private certificateSummary(certificate: Certificate, isIssuedByKnownRoot: boolean, verificationResult: string, errorCode: number): CertificateSummary {
    return {
      subjectName: certificate.subjectName,
      issuerName: certificate.issuerName,
      validStart: certificate.validStart,
      validExpiry: certificate.validExpiry,
      fingerprint: certificate.fingerprint,
      serialNumber: certificate.serialNumber,
      isIssuedByKnownRoot,
      verificationResult,
      errorCode
    }
  }
}
