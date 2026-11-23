# Bundled fonts

Open-licence fonts loaded by `deckcraft-cli` (the headless PowerPoint renderer)
from the `fonts/` folder next to the executable. They provide metric-compatible
substitutes for common Office fonts (Carlito ≈ Calibri, Caladea ≈ Cambria,
Liberation Sans/Serif/Mono ≈ Arial/Times New Roman/Courier New) plus popular
display faces, so slides render identically on machines that lack the requested
fonts. Exact system-installed fonts are still preferred when present.

All fonts are licensed under the SIL Open Font License 1.1 (see each family's
`OFL.txt` / `LICENSE.txt` in its folder). Source: Google Fonts
(https://github.com/google/fonts), except Liberation which is from
https://github.com/liberationfonts (Red Hat, Inc.).

| Family              | Author / foundry              | Metric twin of        | Use |
|---------------------|-------------------------------|-----------------------|-----|
| Carlito             | Łukasz Dziedzic / Google      | Calibri, Calibri Light| Missing Calibri |
| Caladea             | Andrés Torresi, Carolina Giovagnoli / Google | Cambria | Missing Cambria |
| Liberation Sans     | Steve Matteson / Red Hat      | Arial, Helvetica      | Missing Arial |
| Liberation Serif    | Red Hat                       | Times New Roman       | Missing Times |
| Liberation Mono     | Red Hat                       | Courier New           | Missing Courier New |
| Inter               | Rasmus Andersson              | —                     | Generic/Aptos/Segoe UI fallback |
| Lato                | Łukasz Dziedzic               | —                     | Common deck font |
| Merriweather        | Sorkin Type                   | —                     | Common deck font |
| Montserrat          | Julieta Ulanovsky             | —                     | Common deck font |
| Nunito Sans         | Vernon Adams                  | —                     | Common deck font |
| Open Sans           | Steve Matteson / Google       | —                     | Common deck font |
| Oswald              | Vernon Adams                  | —                     | Common deck font |
| Playfair Display    | Claus Eggers Sørensen         | —                     | Common deck font |
| Poppins             | Indian Type Foundry           | —                     | Common deck font |
| PT Sans             | ParaType                      | —                     | Common deck font |
| Raleway             | Matt McInerney et al.         | —                     | Common deck font |
| Roboto              | Christian Robertson / Google  | —                     | Common deck font |
| Bebas Neue          | Ryoichi Tsunekawa / Dharma Type | —                   | Common deck font |
| EB Garamond         | Octavio Pardo                 | —                     | Common deck font |
