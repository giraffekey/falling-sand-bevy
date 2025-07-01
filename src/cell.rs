const DATA_SAND: CellData = CellData {
    material: Material::Powder,
    flammable: false,
    lifespan: None,
    color: [194, 178, 128],
};

const DATA_STONE: CellData = CellData {
    material: Material::Solid,
    flammable: false,
    lifespan: None,
    color: [83, 86, 91],
};

const DATA_WOOD: CellData = CellData {
    material: Material::Solid,
    flammable: true,
    lifespan: None,
    color: [164, 116, 73],
};

const DATA_WATER: CellData = CellData {
    material: Material::Liquid(2),
    flammable: false,
    lifespan: None,
    color: [30, 144, 255],
};

const DATA_OIL: CellData = CellData {
    material: Material::Liquid(1),
    flammable: true,
    lifespan: None,
    color: [59, 49, 49],
};

const DATA_ACID: CellData = CellData {
    material: Material::Acid,
    flammable: false,
    lifespan: None,
    color: [176, 191, 26],
};

const DATA_OXYGEN: CellData = CellData {
    material: Material::Gas,
    flammable: true,
    lifespan: None,
    color: [187, 198, 213],
};

const DATA_FIRE: CellData = CellData {
    material: Material::Fire,
    flammable: false,
    lifespan: Some(20),
    color: [226, 88, 34],
};

const DATA_WIND: CellData = CellData {
    material: Material::Wind,
    flammable: false,
    lifespan: Some(50),
    color: [255, 255, 255],
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    Powder,
    Solid,
    Liquid(u8),
    Acid,
    Gas,
    Fire,
    Wind,
}

#[derive(Debug, Clone, Copy)]
pub struct CellData {
    pub material: Material,
    pub flammable: bool,
    pub lifespan: Option<u8>,
    pub color: [u8; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellId {
    Sand,
    Stone,
    Wood,
    Water,
    Oil,
    Acid,
    Oxygen,
    Fire,
    Wind,
}

impl CellId {
    pub fn data(&self) -> CellData {
        match self {
            CellId::Sand => DATA_SAND,
            CellId::Stone => DATA_STONE,
            CellId::Wood => DATA_WOOD,
            CellId::Water => DATA_WATER,
            CellId::Oil => DATA_OIL,
            CellId::Acid => DATA_ACID,
            CellId::Oxygen => DATA_OXYGEN,
            CellId::Fire => DATA_FIRE,
            CellId::Wind => DATA_WIND,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub id: CellId,
    pub life: Option<u8>,
}

impl Cell {
    pub fn material(&self) -> Material {
        self.id.data().material
    }

    pub fn flammable(&self) -> bool {
        self.id.data().flammable
    }

    pub fn lifespan(&self) -> Option<u8> {
        self.id.data().lifespan
    }

    pub fn color(&self) -> [u8; 3] {
        self.id.data().color
    }

    pub fn falls(&self) -> bool {
        match self.material() {
            Material::Powder | Material::Solid | Material::Liquid(_) | Material::Acid => true,
            Material::Gas | Material::Fire | Material::Wind => false,
        }
    }

    pub fn slides(&self) -> bool {
        match self.material() {
            Material::Powder | Material::Liquid(_) | Material::Acid => true,
            Material::Solid | Material::Gas | Material::Fire | Material::Wind => false,
        }
    }

    pub fn sinks_under(&self, other: Option<Cell>) -> bool {
        match other {
            Some(other) => match (self.material(), other.material()) {
                (Material::Powder, Material::Liquid(_)) => true,
                (Material::Solid, Material::Liquid(_)) => true,
                (Material::Liquid(a), Material::Liquid(b)) => a > b,
                (Material::Powder, Material::Gas) => true,
                (Material::Solid, Material::Gas) => true,
                (Material::Liquid(_), Material::Gas) => true,
                _ => false,
            },
            None => true,
        }
    }

    pub fn dissolves(&self, other: Option<Cell>) -> bool {
        match (self.material(), other.map(|c| c.material())) {
            (Material::Acid, None) => false,
            (Material::Acid, Some(Material::Acid)) => false,
            (Material::Acid, Some(_)) => true,
            _ => false,
        }
    }
}
