# Contrato de pacote e PRELOGIN

HEADER_LENGTH = 8.

Length, incluindo header, packet type, status, channel, PacketID e window são preservados.

PRELOGIN option table define offset/length fora do buffer como erro seguro.

Fixtures mínimas cobrem pacote vazio, fragmentado, truncado e handshake válido.
