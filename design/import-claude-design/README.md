# Import Claude Design — matériau de référence, **à ne pas ouvrir tel quel**

Ces deux fichiers viennent de l'outil de conception qui a servi à explorer la
direction visuelle. Ils ont rempli leur office : la maquette de référence du
projet est [`../keystone-cockpit.html`](../keystone-cockpit.html), qui est la
nôtre et qui est autonome.

## Ce que fait `support.js`, et pourquoi ça compte ici

Il **télécharge et exécute trois scripts depuis un CDN** au chargement de la
page :

```
https://unpkg.com/react@18.3.1/umd/react.production.min.js
https://unpkg.com/react-dom@18.3.1/umd/react-dom.production.min.js
https://unpkg.com/@babel/standalone@7.29.0/babel.min.js
```

Ce n'est pas une ressource passive comme une police : c'est **du code distant
exécuté dans le navigateur**. Ouvrir `Keystone.dc.html` revient donc à faire
confiance à un tiers, à un instant donné, pour le contenu de trois fichiers.

Le principe P5 du projet — « Local, point final » — l'interdit sans nuance pour
une maquette. La règle du dépôt est écrite ainsi : *aucune ressource réseau dans
une maquette ou un rapport ; ni CDN, ni Google Fonts, ni police distante.*

Les trois polices distantes ont été retirées de `Keystone.dc.html`. **Les trois
scripts, non** : les retirer viderait le fichier de sa fonction, puisque c'est
un rendu React. Le fichier est donc conservé **comme archive**, pas comme
maquette ouvrable.

## Ce qu'il faut en faire

Trois options, à trancher :

1. **Supprimer le dossier.** L'import a servi, la maquette de référence existe,
   et rien ici n'est utilisé par le produit. C'est l'option la plus propre.
2. **Le garder hors du dépôt**, dans les archives personnelles du projet.
3. **Le garder ici**, avec cet avertissement — l'option actuelle, choisie par
   défaut plutôt que par décision, ce qui est précisément ce qu'il faut éviter.

En attendant l'arbitrage, considérer ces fichiers comme de la documentation
morte : on les lit dans un éditeur, on ne les ouvre pas dans un navigateur.
