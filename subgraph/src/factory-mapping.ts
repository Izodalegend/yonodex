import { EscrowCreated } from "../generated/YonodexEscrowFactory/YonodexEscrowFactory";
import { YonodexEscrow } from "../generated/templates";
import { Escrow } from "../generated/schema";

export function handleEscrowCreated(event: EscrowCreated): void {
  let escrow = new Escrow(event.params.escrow.toHexString());
  escrow.buyer = event.params.buyer;
  escrow.seller = event.params.seller;
  escrow.salt = event.params.salt;
  escrow.createdAtBlock = event.block.number;
  escrow.status = "CREATED";
  escrow.isFundedBuyer = false;
  escrow.isFundedSeller = false;
  escrow.save();

  // Spawn a template to track events for this specific escrow contract
  YonodexEscrow.create(event.params.escrow);
}