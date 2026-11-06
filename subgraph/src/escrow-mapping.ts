import { Deposited, Settled, Refunded, Cancelled } from "../generated/templates/YonodexEscrow/YonodexEscrow";
import { Escrow } from "../generated/schema";

export function handleDeposited(event: Deposited): void {
  let escrow = Escrow.load(event.address.toHexString());
  if (escrow) {
    if (event.params.depositor == escrow.buyer) {
      escrow.isFundedBuyer = true;
    } else if (event.params.depositor == escrow.seller) {
      escrow.isFundedSeller = true;
    }
    escrow.save();
  }
}

export function handleSettled(event: Settled): void {
  let escrow = Escrow.load(event.address.toHexString());
  if (escrow) {
    escrow.status = "SETTLED";
    escrow.save();
  }
}

export function handleRefunded(event: Refunded): void {
  let escrow = Escrow.load(event.address.toHexString());
  if (escrow) {
    escrow.status = "REFUNDED";
    escrow.save();
  }
}

export function handleCancelled(event: Cancelled): void {
  let escrow = Escrow.load(event.address.toHexString());
  if (escrow) {
    escrow.status = "CANCELLED";
    escrow.save();
  }
}